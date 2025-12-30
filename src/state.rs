use std::{
    collections::{HashMap, VecDeque},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

use chrono::{DateTime, Local};
use parking_lot::RwLock;
use solana_sdk::{clock::Slot, pubkey::Pubkey};

use crate::programs::{KnownPrograms, ProgramCategory, ProgramInfo};

/// Maximum history sizes
const MAX_LOG_ENTRIES: usize = 200;
const MAX_SLOT_HISTORY: usize = 100;
const MAX_TXN_SAMPLES: usize = 50;
const MAX_LATENCY_SAMPLES: usize = 100;
const MAX_LEADER_HISTORY: usize = 50;
const MAX_BUNDLE_SAMPLES: usize = 50;

// ============================================================================
// Connection State
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    Error(String),
}

impl std::fmt::Display for ConnectionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConnectionState::Disconnected => write!(f, "Disconnected"),
            ConnectionState::Connecting => write!(f, "Connecting..."),
            ConnectionState::Connected => write!(f, "Connected"),
            ConnectionState::Reconnecting => write!(f, "Reconnecting..."),
            ConnectionState::Error(e) => write!(f, "Error: {}", e),
        }
    }
}

// ============================================================================
// Logging
// ============================================================================

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: DateTime<Local>,
    pub level: LogLevel,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Warn,
    Error,
    Debug,
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogLevel::Info => write!(f, "INFO"),
            LogLevel::Warn => write!(f, "WARN"),
            LogLevel::Error => write!(f, "ERROR"),
            LogLevel::Debug => write!(f, "DEBUG"),
        }
    }
}

// ============================================================================
// Slot & Entry Tracking
// ============================================================================

#[derive(Debug, Clone)]
pub struct SlotInfo {
    pub slot: Slot,
    pub entry_count: u64,
    pub txn_count: u64,
    pub received_at: Instant,
    pub timestamp: DateTime<Local>,
    pub first_shred_delay_ms: Option<f64>,
    pub leader: Option<Pubkey>,
    pub dex_txn_count: u64,
    pub jito_bundle_count: u64,
    pub turbine_index: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct TxnSample {
    pub slot: Slot,
    pub signature: String,
    pub received_at: DateTime<Local>,
    pub programs: Vec<String>,
    pub is_bundle: bool,
    pub tip_amount: Option<u64>,
}

// ============================================================================
// Latency Tracking
// ============================================================================

#[derive(Debug, Clone)]
pub struct LatencySample {
    pub slot: Slot,
    pub timestamp: DateTime<Local>,
    pub shred_latency_us: u64,
    pub leader: Option<Pubkey>,
    pub region: Option<String>,
    pub turbine_index: Option<u32>,
}

#[derive(Debug, Default)]
pub struct LatencyStats {
    pub samples: RwLock<VecDeque<LatencySample>>,
    pub min_latency_us: AtomicU64,
    pub max_latency_us: AtomicU64,
    pub total_latency_us: AtomicU64,
    pub sample_count: AtomicU64,
    pub leader_latencies: RwLock<HashMap<Pubkey, LeaderLatencyStats>>,
    pub region_latencies: RwLock<HashMap<String, RegionLatencyStats>>,
}

#[derive(Debug, Clone, Default)]
pub struct LeaderLatencyStats {
    pub leader: Pubkey,
    pub total_latency_us: u64,
    pub sample_count: u64,
    pub min_latency_us: u64,
    pub max_latency_us: u64,
}

impl LeaderLatencyStats {
    pub fn avg_latency_ms(&self) -> f64 {
        if self.sample_count == 0 {
            0.0
        } else {
            (self.total_latency_us as f64 / self.sample_count as f64) / 1000.0
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct RegionLatencyStats {
    pub region: String,
    pub total_latency_us: u64,
    pub sample_count: u64,
    pub min_latency_us: u64,
    pub max_latency_us: u64,
}

impl RegionLatencyStats {
    pub fn avg_latency_ms(&self) -> f64 {
        if self.sample_count == 0 {
            0.0
        } else {
            (self.total_latency_us as f64 / self.sample_count as f64) / 1000.0
        }
    }
}

impl LatencyStats {
    pub fn new() -> Self {
        Self {
            samples: RwLock::new(VecDeque::with_capacity(MAX_LATENCY_SAMPLES)),
            min_latency_us: AtomicU64::new(u64::MAX),
            max_latency_us: AtomicU64::new(0),
            total_latency_us: AtomicU64::new(0),
            sample_count: AtomicU64::new(0),
            leader_latencies: RwLock::new(HashMap::new()),
            region_latencies: RwLock::new(HashMap::new()),
        }
    }

    pub fn add_sample(&self, sample: LatencySample) {
        let latency = sample.shred_latency_us;
        
        self.total_latency_us.fetch_add(latency, Ordering::Relaxed);
        self.sample_count.fetch_add(1, Ordering::Relaxed);
        
        // Update min
        let mut current_min = self.min_latency_us.load(Ordering::Relaxed);
        while latency < current_min {
            match self.min_latency_us.compare_exchange_weak(
                current_min, latency, Ordering::Relaxed, Ordering::Relaxed
            ) {
                Ok(_) => break,
                Err(x) => current_min = x,
            }
        }
        
        // Update max
        let mut current_max = self.max_latency_us.load(Ordering::Relaxed);
        while latency > current_max {
            match self.max_latency_us.compare_exchange_weak(
                current_max, latency, Ordering::Relaxed, Ordering::Relaxed
            ) {
                Ok(_) => break,
                Err(x) => current_max = x,
            }
        }
        
        // Update leader stats
        if let Some(leader) = sample.leader {
            let mut leader_stats = self.leader_latencies.write();
            let stats = leader_stats.entry(leader).or_insert_with(|| LeaderLatencyStats {
                leader,
                ..Default::default()
            });
            stats.total_latency_us += latency;
            stats.sample_count += 1;
            if latency < stats.min_latency_us || stats.min_latency_us == 0 {
                stats.min_latency_us = latency;
            }
            if latency > stats.max_latency_us {
                stats.max_latency_us = latency;
            }
        }
        
        // Update region stats
        if let Some(ref region) = sample.region {
            let mut region_stats = self.region_latencies.write();
            let stats = region_stats.entry(region.clone()).or_insert_with(|| RegionLatencyStats {
                region: region.clone(),
                ..Default::default()
            });
            stats.total_latency_us += latency;
            stats.sample_count += 1;
            if latency < stats.min_latency_us || stats.min_latency_us == 0 {
                stats.min_latency_us = latency;
            }
            if latency > stats.max_latency_us {
                stats.max_latency_us = latency;
            }
        }
        
        let mut samples = self.samples.write();
        if samples.len() >= MAX_LATENCY_SAMPLES {
            samples.pop_front();
        }
        samples.push_back(sample);
    }

    pub fn avg_latency_ms(&self) -> f64 {
        let count = self.sample_count.load(Ordering::Relaxed);
        if count == 0 {
            return 0.0;
        }
        let total = self.total_latency_us.load(Ordering::Relaxed);
        (total as f64 / count as f64) / 1000.0
    }

    pub fn min_latency_ms(&self) -> f64 {
        let min = self.min_latency_us.load(Ordering::Relaxed);
        if min == u64::MAX { 0.0 } else { min as f64 / 1000.0 }
    }

    pub fn max_latency_ms(&self) -> f64 {
        self.max_latency_us.load(Ordering::Relaxed) as f64 / 1000.0
    }
}

// ============================================================================
// Program Activity Tracking
// ============================================================================

#[derive(Debug, Clone)]
pub struct ProgramActivity {
    pub program_id: Pubkey,
    pub name: String,
    pub category: ProgramCategory,
    pub txn_count: u64,
    pub last_seen: DateTime<Local>,
}

#[derive(Debug)]
pub struct ProgramStats {
    pub activities: RwLock<HashMap<Pubkey, ProgramActivity>>,
    pub known_programs: HashMap<Pubkey, ProgramInfo>,
    pub dex_txn_count: AtomicU64,
    pub lending_txn_count: AtomicU64,
    pub mev_txn_count: AtomicU64,
    pub staking_txn_count: AtomicU64,
}

impl Default for ProgramStats {
    fn default() -> Self {
        Self::new()
    }
}

impl ProgramStats {
    pub fn new() -> Self {
        Self {
            activities: RwLock::new(HashMap::new()),
            known_programs: KnownPrograms::get_all(),
            dex_txn_count: AtomicU64::new(0),
            lending_txn_count: AtomicU64::new(0),
            mev_txn_count: AtomicU64::new(0),
            staking_txn_count: AtomicU64::new(0),
        }
    }

    pub fn record_program(&self, program_id: Pubkey) {
        let mut activities = self.activities.write();
        
        let (name, category) = if let Some(info) = self.known_programs.get(&program_id) {
            (info.name.clone(), info.category)
        } else {
            (program_id.to_string()[..8].to_string(), ProgramCategory::Other)
        };
        
        match category {
            ProgramCategory::Dex => { self.dex_txn_count.fetch_add(1, Ordering::Relaxed); }
            ProgramCategory::Lending => { self.lending_txn_count.fetch_add(1, Ordering::Relaxed); }
            ProgramCategory::Mev => { self.mev_txn_count.fetch_add(1, Ordering::Relaxed); }
            ProgramCategory::Staking => { self.staking_txn_count.fetch_add(1, Ordering::Relaxed); }
            _ => {}
        }
        
        activities.entry(program_id)
            .and_modify(|a| {
                a.txn_count += 1;
                a.last_seen = Local::now();
            })
            .or_insert_with(|| ProgramActivity {
                program_id,
                name,
                category,
                txn_count: 1,
                last_seen: Local::now(),
            });
    }

    pub fn get_top_programs(&self, limit: usize) -> Vec<ProgramActivity> {
        let activities = self.activities.read();
        let mut programs: Vec<_> = activities.values().cloned().collect();
        programs.sort_by(|a, b| b.txn_count.cmp(&a.txn_count));
        programs.truncate(limit);
        programs
    }
}

// ============================================================================
// Leader Tracking
// ============================================================================

#[derive(Debug, Clone)]
pub struct LeaderSlotInfo {
    pub slot: Slot,
    pub leader: Pubkey,
    pub entry_count: u64,
    pub txn_count: u64,
    pub skip: bool,
    pub first_shred_delay_ms: Option<f64>,
    pub timestamp: DateTime<Local>,
}

#[derive(Debug, Clone, Default)]
pub struct LeaderStats {
    pub leader: Pubkey,
    pub slots_seen: u64,
    pub slots_skipped: u64,
    pub total_txns: u64,
    pub avg_latency_ms: f64,
}

impl LeaderStats {
    pub fn skip_rate(&self) -> f64 {
        if self.slots_seen == 0 {
            0.0
        } else {
            (self.slots_skipped as f64 / self.slots_seen as f64) * 100.0
        }
    }
}

#[derive(Debug, Default)]
pub struct LeaderTracker {
    pub slot_history: RwLock<VecDeque<LeaderSlotInfo>>,
    pub leader_stats: RwLock<HashMap<Pubkey, LeaderStats>>,
    pub current_leader: RwLock<Option<Pubkey>>,
    pub upcoming_leaders: RwLock<Vec<(Slot, Pubkey)>>,
}

impl LeaderTracker {
    pub fn new() -> Self {
        Self {
            slot_history: RwLock::new(VecDeque::with_capacity(MAX_LEADER_HISTORY)),
            leader_stats: RwLock::new(HashMap::new()),
            current_leader: RwLock::new(None),
            upcoming_leaders: RwLock::new(Vec::new()),
        }
    }

    pub fn record_slot(&self, info: LeaderSlotInfo) {
        *self.current_leader.write() = Some(info.leader);
        
        {
            let mut stats = self.leader_stats.write();
            let leader_stat = stats.entry(info.leader).or_insert_with(|| LeaderStats {
                leader: info.leader,
                ..Default::default()
            });
            leader_stat.slots_seen += 1;
            if info.skip {
                leader_stat.slots_skipped += 1;
            }
            leader_stat.total_txns += info.txn_count;
        }
        
        let mut history = self.slot_history.write();
        if history.len() >= MAX_LEADER_HISTORY {
            history.pop_front();
        }
        history.push_back(info);
    }

    pub fn get_top_leaders(&self, limit: usize) -> Vec<LeaderStats> {
        let stats = self.leader_stats.read();
        let mut leaders: Vec<_> = stats.values().cloned().collect();
        leaders.sort_by(|a, b| b.slots_seen.cmp(&a.slots_seen));
        leaders.truncate(limit);
        leaders
    }
}

// ============================================================================
// Turbine Tree Tracking
// ============================================================================

#[derive(Debug, Clone)]
pub struct TurbineInfo {
    pub slot: Slot,
    pub shred_index: u32,
    pub turbine_index: u32,
    pub layer: u32,
    pub timestamp: DateTime<Local>,
    pub source_ip: Option<String>,
}

#[derive(Debug, Default)]
pub struct TurbineStats {
    pub samples: RwLock<VecDeque<TurbineInfo>>,
    pub total_samples: AtomicU64,
    pub sum_index: AtomicU64,
    pub min_index: AtomicU64,
    pub max_index: AtomicU64,
    pub layer_0_count: AtomicU64,
    pub layer_1_count: AtomicU64,
    pub layer_2_count: AtomicU64,
    pub layer_3_plus_count: AtomicU64,
}

impl TurbineStats {
    pub fn new() -> Self {
        Self {
            samples: RwLock::new(VecDeque::with_capacity(MAX_LATENCY_SAMPLES)),
            total_samples: AtomicU64::new(0),
            sum_index: AtomicU64::new(0),
            min_index: AtomicU64::new(u64::MAX),
            max_index: AtomicU64::new(0),
            layer_0_count: AtomicU64::new(0),
            layer_1_count: AtomicU64::new(0),
            layer_2_count: AtomicU64::new(0),
            layer_3_plus_count: AtomicU64::new(0),
        }
    }

    pub fn add_sample(&self, info: TurbineInfo) {
        let index = info.turbine_index as u64;
        
        self.total_samples.fetch_add(1, Ordering::Relaxed);
        self.sum_index.fetch_add(index, Ordering::Relaxed);
        
        // Update min
        let mut current_min = self.min_index.load(Ordering::Relaxed);
        while index < current_min {
            match self.min_index.compare_exchange_weak(
                current_min, index, Ordering::Relaxed, Ordering::Relaxed
            ) {
                Ok(_) => break,
                Err(x) => current_min = x,
            }
        }
        
        // Update max
        let mut current_max = self.max_index.load(Ordering::Relaxed);
        while index > current_max {
            match self.max_index.compare_exchange_weak(
                current_max, index, Ordering::Relaxed, Ordering::Relaxed
            ) {
                Ok(_) => break,
                Err(x) => current_max = x,
            }
        }
        
        match info.layer {
            0 => self.layer_0_count.fetch_add(1, Ordering::Relaxed),
            1 => self.layer_1_count.fetch_add(1, Ordering::Relaxed),
            2 => self.layer_2_count.fetch_add(1, Ordering::Relaxed),
            _ => self.layer_3_plus_count.fetch_add(1, Ordering::Relaxed),
        };
        
        let mut samples = self.samples.write();
        if samples.len() >= MAX_LATENCY_SAMPLES {
            samples.pop_front();
        }
        samples.push_back(info);
    }

    pub fn avg_index(&self) -> f64 {
        let count = self.total_samples.load(Ordering::Relaxed);
        if count == 0 { return 0.0; }
        self.sum_index.load(Ordering::Relaxed) as f64 / count as f64
    }

    pub fn min_index(&self) -> u64 {
        let min = self.min_index.load(Ordering::Relaxed);
        if min == u64::MAX { 0 } else { min }
    }

    pub fn max_index(&self) -> u64 {
        self.max_index.load(Ordering::Relaxed)
    }
}

// ============================================================================
// Bundle & Competition Detection
// ============================================================================

#[derive(Debug, Clone)]
pub struct BundleInfo {
    pub slot: Slot,
    pub txn_count: u32,
    pub tip_amount: u64,
    pub tip_account: String,
    pub signatures: Vec<String>,
    pub timestamp: DateTime<Local>,
}

#[derive(Debug, Clone)]
pub struct SandwichPattern {
    pub slot: Slot,
    pub victim_sig: String,
    pub frontrun_sig: String,
    pub backrun_sig: String,
    pub timestamp: DateTime<Local>,
}

#[derive(Debug, Default)]
pub struct CompetitionStats {
    pub bundles: RwLock<VecDeque<BundleInfo>>,
    pub sandwiches: RwLock<VecDeque<SandwichPattern>>,
    pub duplicate_txns: RwLock<VecDeque<String>>,
    pub bundle_count: AtomicU64,
    pub total_tips_lamports: AtomicU64,
    pub sandwich_count: AtomicU64,
    pub duplicate_count: AtomicU64,
}

impl CompetitionStats {
    pub fn new() -> Self {
        Self {
            bundles: RwLock::new(VecDeque::with_capacity(MAX_BUNDLE_SAMPLES)),
            sandwiches: RwLock::new(VecDeque::with_capacity(MAX_BUNDLE_SAMPLES)),
            duplicate_txns: RwLock::new(VecDeque::with_capacity(MAX_TXN_SAMPLES)),
            bundle_count: AtomicU64::new(0),
            total_tips_lamports: AtomicU64::new(0),
            sandwich_count: AtomicU64::new(0),
            duplicate_count: AtomicU64::new(0),
        }
    }

    pub fn add_bundle(&self, bundle: BundleInfo) {
        self.bundle_count.fetch_add(1, Ordering::Relaxed);
        self.total_tips_lamports.fetch_add(bundle.tip_amount, Ordering::Relaxed);
        
        let mut bundles = self.bundles.write();
        if bundles.len() >= MAX_BUNDLE_SAMPLES {
            bundles.pop_front();
        }
        bundles.push_back(bundle);
    }

    pub fn total_tips_sol(&self) -> f64 {
        self.total_tips_lamports.load(Ordering::Relaxed) as f64 / 1_000_000_000.0
    }
}

// ============================================================================
// Wallet Monitoring
// ============================================================================

#[derive(Debug, Clone)]
pub struct WalletTxn {
    pub slot: Slot,
    pub signature: String,
    pub timestamp: DateTime<Local>,
    pub success: bool,
    pub programs: Vec<String>,
}

#[derive(Debug, Default)]
pub struct WalletMonitor {
    pub wallet: RwLock<Option<Pubkey>>,
    pub transactions: RwLock<VecDeque<WalletTxn>>,
    pub txn_count: AtomicU64,
    pub success_count: AtomicU64,
    pub fail_count: AtomicU64,
}

impl WalletMonitor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_wallet(&self, wallet: Pubkey) {
        *self.wallet.write() = Some(wallet);
    }

    pub fn add_txn(&self, txn: WalletTxn) {
        self.txn_count.fetch_add(1, Ordering::Relaxed);
        if txn.success {
            self.success_count.fetch_add(1, Ordering::Relaxed);
        } else {
            self.fail_count.fetch_add(1, Ordering::Relaxed);
        }
        
        let mut txns = self.transactions.write();
        if txns.len() >= MAX_TXN_SAMPLES {
            txns.pop_front();
        }
        txns.push_back(txn);
    }
}

// ============================================================================
// Network Health
// ============================================================================

#[derive(Debug, Default)]
pub struct NetworkHealth {
    pub fec_recovery_count: AtomicU64,
    pub direct_receive_count: AtomicU64,
    pub missed_slots: RwLock<VecDeque<Slot>>,
    pub heartbeat_success: AtomicU64,
    pub heartbeat_fail: AtomicU64,
}

impl NetworkHealth {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn fec_recovery_rate(&self) -> f64 {
        let recovered = self.fec_recovery_count.load(Ordering::Relaxed);
        let direct = self.direct_receive_count.load(Ordering::Relaxed);
        let total = recovered + direct;
        if total == 0 { 0.0 } else { (recovered as f64 / total as f64) * 100.0 }
    }

    pub fn heartbeat_success_rate(&self) -> f64 {
        let success = self.heartbeat_success.load(Ordering::Relaxed);
        let fail = self.heartbeat_fail.load(Ordering::Relaxed);
        let total = success + fail;
        if total == 0 { 100.0 } else { (success as f64 / total as f64) * 100.0 }
    }
}

// ============================================================================
// Shred Metrics
// ============================================================================

#[derive(Debug, Default)]
pub struct ShredMetrics {
    pub received: AtomicU64,
    pub success_forward: AtomicU64,
    pub fail_forward: AtomicU64,
    pub duplicate: AtomicU64,
    pub recovered_count: AtomicU64,
    pub entry_count: AtomicU64,
    pub txn_count: AtomicU64,
    pub total_received: AtomicU64,
    pub total_success_forward: AtomicU64,
    pub total_fail_forward: AtomicU64,
    pub total_duplicate: AtomicU64,
    pub total_entries: AtomicU64,
    pub total_txns: AtomicU64,
}

impl ShredMetrics {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_entry(&self, entry_count: u64, txn_count: u64) {
        self.entry_count.fetch_add(entry_count, Ordering::Relaxed);
        self.txn_count.fetch_add(txn_count, Ordering::Relaxed);
        self.total_entries.fetch_add(entry_count, Ordering::Relaxed);
        self.total_txns.fetch_add(txn_count, Ordering::Relaxed);
    }

    pub fn get_entries_per_sec(&self, duration_secs: f64) -> f64 {
        if duration_secs <= 0.0 { return 0.0; }
        self.entry_count.load(Ordering::Relaxed) as f64 / duration_secs
    }

    pub fn get_txns_per_sec(&self, duration_secs: f64) -> f64 {
        if duration_secs <= 0.0 { return 0.0; }
        self.txn_count.load(Ordering::Relaxed) as f64 / duration_secs
    }

    pub fn reset_window(&self) {
        self.received.store(0, Ordering::Relaxed);
        self.success_forward.store(0, Ordering::Relaxed);
        self.fail_forward.store(0, Ordering::Relaxed);
        self.duplicate.store(0, Ordering::Relaxed);
        self.entry_count.store(0, Ordering::Relaxed);
        self.txn_count.store(0, Ordering::Relaxed);
        self.recovered_count.store(0, Ordering::Relaxed);
    }
}

// ============================================================================
// Main Application State
// ============================================================================

pub struct AppState {
    pub proxy_url: String,
    pub connection_state: RwLock<ConnectionState>,
    pub connected_at: RwLock<Option<Instant>>,
    pub reconnect_count: AtomicU64,

    pub metrics: ShredMetrics,
    pub metrics_window_start: RwLock<Instant>,

    pub current_slot: AtomicU64,
    pub slot_history: RwLock<VecDeque<SlotInfo>>,
    pub txn_samples: RwLock<VecDeque<TxnSample>>,

    pub latency_stats: LatencyStats,
    pub program_stats: ProgramStats,
    pub leader_tracker: LeaderTracker,
    pub turbine_stats: TurbineStats,
    pub competition_stats: CompetitionStats,
    pub wallet_monitor: WalletMonitor,
    pub network_health: NetworkHealth,

    pub logs: RwLock<VecDeque<LogEntry>>,

    pub selected_tab: RwLock<usize>,
    pub scroll_offset: RwLock<usize>,
    pub show_help: RwLock<bool>,

    pub start_time: Instant,
}

impl AppState {
    pub fn new(proxy_url: String) -> Self {
        Self {
            proxy_url,
            connection_state: RwLock::new(ConnectionState::Disconnected),
            connected_at: RwLock::new(None),
            reconnect_count: AtomicU64::new(0),
            metrics: ShredMetrics::new(),
            metrics_window_start: RwLock::new(Instant::now()),
            current_slot: AtomicU64::new(0),
            slot_history: RwLock::new(VecDeque::with_capacity(MAX_SLOT_HISTORY)),
            txn_samples: RwLock::new(VecDeque::with_capacity(MAX_TXN_SAMPLES)),
            latency_stats: LatencyStats::new(),
            program_stats: ProgramStats::new(),
            leader_tracker: LeaderTracker::new(),
            turbine_stats: TurbineStats::new(),
            competition_stats: CompetitionStats::new(),
            wallet_monitor: WalletMonitor::new(),
            network_health: NetworkHealth::new(),
            logs: RwLock::new(VecDeque::with_capacity(MAX_LOG_ENTRIES)),
            selected_tab: RwLock::new(0),
            scroll_offset: RwLock::new(0),
            show_help: RwLock::new(false),
            start_time: Instant::now(),
        }
    }

    pub fn log(&self, level: LogLevel, message: impl Into<String>) {
        let mut logs = self.logs.write();
        if logs.len() >= MAX_LOG_ENTRIES {
            logs.pop_front();
        }
        logs.push_back(LogEntry {
            timestamp: Local::now(),
            level,
            message: message.into(),
        });
    }

    pub fn log_info(&self, message: impl Into<String>) {
        self.log(LogLevel::Info, message);
    }

    pub fn log_warn(&self, message: impl Into<String>) {
        self.log(LogLevel::Warn, message);
    }

    pub fn log_error(&self, message: impl Into<String>) {
        self.log(LogLevel::Error, message);
    }

    pub fn set_connection_state(&self, state: ConnectionState) {
        let mut conn_state = self.connection_state.write();
        if *conn_state != state {
            self.log_info(format!("Connection state: {}", state));
            *conn_state = state.clone();
            if state == ConnectionState::Connected {
                *self.connected_at.write() = Some(Instant::now());
            }
        }
    }

    pub fn add_slot(&self, slot: Slot, entry_count: u64, txn_count: u64) {
        let current = self.current_slot.load(Ordering::Relaxed);
        if slot > current {
            self.current_slot.store(slot, Ordering::Relaxed);
        }

        let mut history = self.slot_history.write();
        if history.len() >= MAX_SLOT_HISTORY {
            history.pop_front();
        }
        history.push_back(SlotInfo {
            slot,
            entry_count,
            txn_count,
            received_at: Instant::now(),
            timestamp: Local::now(),
            first_shred_delay_ms: None,
            leader: None,
            dex_txn_count: 0,
            jito_bundle_count: 0,
            turbine_index: None,
        });

        self.metrics.add_entry(entry_count, txn_count);
    }

    pub fn add_txn_sample(&self, slot: Slot, signature: String, programs: Vec<String>, is_bundle: bool, tip_amount: Option<u64>) {
        let mut samples = self.txn_samples.write();
        if samples.len() >= MAX_TXN_SAMPLES {
            samples.pop_front();
        }
        samples.push_back(TxnSample {
            slot,
            signature,
            received_at: Local::now(),
            programs,
            is_bundle,
            tip_amount,
        });
    }

    pub fn uptime(&self) -> Duration {
        self.start_time.elapsed()
    }

    pub fn connection_duration(&self) -> Option<Duration> {
        self.connected_at.read().map(|t| t.elapsed())
    }

    pub fn metrics_window_secs(&self) -> f64 {
        self.metrics_window_start.read().elapsed().as_secs_f64()
    }

    pub fn reset_metrics_window(&self) {
        *self.metrics_window_start.write() = Instant::now();
        self.metrics.reset_window();
    }

    pub fn next_tab(&self) {
        let mut tab = self.selected_tab.write();
        *tab = (*tab + 1) % 8;
    }

    pub fn prev_tab(&self) {
        let mut tab = self.selected_tab.write();
        *tab = if *tab == 0 { 7 } else { *tab - 1 };
    }

    pub fn toggle_help(&self) {
        let mut show = self.show_help.write();
        *show = !*show;
    }

    pub fn scroll_up(&self) {
        let mut offset = self.scroll_offset.write();
        *offset = offset.saturating_sub(1);
    }

    pub fn scroll_down(&self) {
        let mut offset = self.scroll_offset.write();
        *offset = offset.saturating_add(1);
    }
}
