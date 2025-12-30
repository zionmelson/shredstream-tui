use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use chrono::Local;
use jito_protos::shredstream::{
    shredstream_proxy_client::ShredstreamProxyClient,
    SubscribeEntriesRequest,
};
use solana_entry::entry::Entry;
use solana_sdk::pubkey::Pubkey;
use tokio::sync::mpsc;
use tokio_stream::StreamExt;
use tonic::transport::Channel;

use crate::programs::{JITO_TIP_ACCOUNTS, KnownPrograms};
use crate::state::{AppState, BundleInfo, ConnectionState};

/// Message types from the client to the main app
#[derive(Debug, Clone)]
pub enum ClientMessage {
    EntriesReceived {
        slot: u64,
        entry_count: usize,
        txn_count: usize,
    },
    ConnectionChanged(ConnectionState),
    Error(String),
}

/// ShredStream client for connecting to the proxy's gRPC service
pub struct ShredstreamClient {
    proxy_url: String,
    state: Arc<AppState>,
}

impl ShredstreamClient {
    pub fn new(proxy_url: String, state: Arc<AppState>) -> Self {
        Self { proxy_url, state }
    }

    async fn create_channel(&self) -> Result<Channel> {
        let endpoint = tonic::transport::Endpoint::from_shared(self.proxy_url.clone())
            .context("Invalid proxy URL")?
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(60));
        
        endpoint.connect().await.context("Failed to connect to proxy")
    }

    pub async fn subscribe(&self, tx: mpsc::Sender<ClientMessage>) -> Result<()> {
        loop {
            self.state.set_connection_state(ConnectionState::Connecting);
            
            match self.try_subscribe(&tx).await {
                Ok(_) => {
                    self.state.log_info("Stream ended, reconnecting...");
                }
                Err(e) => {
                    self.state.log_error(format!("Connection error: {}", e));
                    let _ = tx.send(ClientMessage::Error(e.to_string())).await;
                }
            }

            self.state.set_connection_state(ConnectionState::Reconnecting);
            self.state.reconnect_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    }

    async fn try_subscribe(&self, tx: &mpsc::Sender<ClientMessage>) -> Result<()> {
        let channel = self.create_channel().await?;
        let mut client = ShredstreamProxyClient::new(channel);

        self.state.log_info(format!("Connected to proxy at {}", self.proxy_url));
        self.state.set_connection_state(ConnectionState::Connected);
        let _ = tx.send(ClientMessage::ConnectionChanged(ConnectionState::Connected)).await;

        let request = tonic::Request::new(SubscribeEntriesRequest {});
        let response = client.subscribe_entries(request).await?;
        let mut stream = response.into_inner();

        // Track seen signatures for duplicate detection
        let mut recent_sigs: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut sig_cleanup_counter = 0u64;

        // Jito tip accounts as pubkeys
        let jito_tip_pubkeys: Vec<Pubkey> = JITO_TIP_ACCOUNTS
            .iter()
            .filter_map(|s| s.parse().ok())
            .collect();

        // Known program lookup
        let known_programs = KnownPrograms::get_all();

        while let Some(result) = stream.next().await {
            match result {
                Ok(entry_pb) => {
                    match bincode::deserialize::<Vec<Entry>>(&entry_pb.entries) {
                        Ok(entries) => {
                            let slot = entry_pb.slot;
                            let entry_count = entries.len();
                            let txn_count: usize = entries.iter()
                                .map(|e| e.transactions.len())
                                .sum();

                            // Track DEX and bundle activity
                            let mut dex_count = 0u64;
                            let mut bundle_count = 0u64;
                            let mut bundle_txns: Vec<String> = Vec::new();
                            let mut bundle_tip: u64 = 0;
                            let mut bundle_tip_account = String::new();

                            for entry in &entries {
                                for txn in &entry.transactions {
                                    if txn.signatures.is_empty() {
                                        continue;
                                    }
                                    
                                    let sig = txn.signatures[0].to_string();
                                    
                                    // Duplicate detection
                                    if recent_sigs.contains(&sig) {
                                        self.state.competition_stats.duplicate_count
                                            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                    } else {
                                        recent_sigs.insert(sig.clone());
                                    }

                                    // Extract program IDs from transaction
                                    let mut program_names: Vec<String> = Vec::new();
                                    let mut is_dex = false;
                                    let mut is_jito_tip = false;
                                    let mut tip_amount: Option<u64> = None;

                                    // Check account keys for programs and tip accounts
                                    let account_keys = txn.message.static_account_keys();
                                    for key in account_keys.iter() {
                                        // Check if it's a Jito tip account
                                        if jito_tip_pubkeys.contains(key) {
                                            is_jito_tip = true;
                                            bundle_tip_account = key.to_string();
                                            // Note: Would need to parse instruction data for actual tip amount
                                        }

                                        // Check if it's a known program
                                        if let Some(info) = known_programs.get(key) {
                                            program_names.push(info.name.clone());
                                            self.state.program_stats.record_program(*key);
                                            
                                            if matches!(info.category, crate::programs::ProgramCategory::Dex) {
                                                is_dex = true;
                                            }
                                        }
                                    }

                                    if is_dex {
                                        dex_count += 1;
                                    }

                                    if is_jito_tip {
                                        bundle_count += 1;
                                        bundle_txns.push(sig.clone());
                                    }

                                    // Sample transactions (prioritize interesting ones)
                                    let should_sample = is_dex || is_jito_tip || 
                                        self.state.txn_samples.read().len() < 10;
                                    
                                    if should_sample {
                                        self.state.add_txn_sample(
                                            slot,
                                            sig,
                                            program_names,
                                            is_jito_tip,
                                            tip_amount,
                                        );
                                    }

                                    // Check if transaction involves monitored wallet
                                    if let Some(wallet) = *self.state.wallet_monitor.wallet.read() {
                                        for key in account_keys.iter() {
                                            if key == &wallet {
                                                self.state.wallet_monitor.add_txn(
                                                    crate::state::WalletTxn {
                                                        slot,
                                                        signature: txn.signatures[0].to_string(),
                                                        timestamp: Local::now(),
                                                        success: true, // Can't determine from shred data
                                                        programs: Vec::new(),
                                                    }
                                                );
                                                break;
                                            }
                                        }
                                    }
                                }
                            }

                            // Record bundle if detected
                            if bundle_count > 0 && !bundle_txns.is_empty() {
                                self.state.competition_stats.add_bundle(BundleInfo {
                                    slot,
                                    txn_count: bundle_txns.len() as u32,
                                    tip_amount: bundle_tip,
                                    tip_account: bundle_tip_account,
                                    signatures: bundle_txns,
                                    timestamp: Local::now(),
                                });
                            }

                            // Update slot info
                            self.state.add_slot(slot, entry_count as u64, txn_count as u64);

                            // Send to main app
                            let _ = tx.send(ClientMessage::EntriesReceived {
                                slot,
                                entry_count,
                                txn_count,
                            }).await;

                            // Periodic cleanup of seen signatures (every 1000 entries)
                            sig_cleanup_counter += 1;
                            if sig_cleanup_counter % 1000 == 0 && recent_sigs.len() > 50000 {
                                recent_sigs.clear();
                            }
                        }
                        Err(e) => {
                            self.state.log_warn(format!(
                                "Failed to deserialize entries for slot {}: {}",
                                entry_pb.slot, e
                            ));
                        }
                    }
                }
                Err(e) => {
                    return Err(anyhow::anyhow!("Stream error: {}", e));
                }
            }
        }

        Ok(())
    }
}

/// Start the client in a background task
pub fn start_client(
    proxy_url: String,
    state: Arc<AppState>,
    tx: mpsc::Sender<ClientMessage>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let client = ShredstreamClient::new(proxy_url, state);
        if let Err(e) = client.subscribe(tx).await {
            tracing::error!("Client fatal error: {}", e);
        }
    })
}
