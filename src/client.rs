use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use jito_protos::shredstream::{
    shredstream_proxy_client::ShredstreamProxyClient,
    SubscribeEntriesRequest,
};
use solana_entry::entry::Entry;
use tokio::sync::mpsc;
use tokio_stream::StreamExt;
use tonic::transport::Channel;

use crate::state::{AppState, ConnectionState};

/// Message types from the client to the main app
#[derive(Debug, Clone)]
pub enum ClientMessage {
    /// Received entries from the proxy
    EntriesReceived {
        slot: u64,
        entries: Vec<Entry>,
    },
    /// Connection state changed
    ConnectionChanged(ConnectionState),
    /// Error occurred
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

    /// Create a gRPC channel to the proxy
    async fn create_channel(&self) -> Result<Channel> {
        let endpoint = tonic::transport::Endpoint::from_shared(self.proxy_url.clone())
            .context("Invalid proxy URL")?
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(60));
        
        endpoint.connect().await.context("Failed to connect to proxy")
    }

    /// Subscribe to entries and send them to the provided channel
    pub async fn subscribe(&self, tx: mpsc::Sender<ClientMessage>) -> Result<()> {
        loop {
            self.state.set_connection_state(ConnectionState::Connecting);
            
            match self.try_subscribe(&tx).await {
                Ok(_) => {
                    // Stream ended normally
                    self.state.log_info("Stream ended, reconnecting...");
                }
                Err(e) => {
                    self.state.log_error(format!("Connection error: {}", e));
                    let _ = tx.send(ClientMessage::Error(e.to_string())).await;
                }
            }

            self.state.set_connection_state(ConnectionState::Reconnecting);
            self.state.reconnect_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            
            // Wait before reconnecting
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

        while let Some(result) = stream.next().await {
            match result {
                Ok(entry_pb) => {
                    // Deserialize the entries
                    match bincode::deserialize::<Vec<Entry>>(&entry_pb.entries) {
                        Ok(entries) => {
                            let slot = entry_pb.slot;
                            
                            // Log some sample transactions
                            for entry in &entries {
                                for txn in &entry.transactions {
                                    if !txn.signatures.is_empty() {
                                        self.state.add_txn_sample(
                                            slot,
                                            txn.signatures[0].to_string(),
                                        );
                                    }
                                }
                            }

                            // Update slot info
                            let entry_count = entries.len() as u64;
                            let txn_count: u64 = entries.iter()
                                .map(|e| e.transactions.len() as u64)
                                .sum();
                            
                            self.state.add_slot(slot, entry_count, txn_count);

                            // Send to main app
                            let _ = tx.send(ClientMessage::EntriesReceived {
                                slot,
                                entries,
                            }).await;
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
