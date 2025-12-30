use std::{
    collections::VecDeque,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

use chrono::{DateTime, Local};
use parking_lot::RwLock;
use solana_sdk::clock::Slot;

/// Maximum number of log entries to keep
const MAX_LOG_ENTRIES: usize = 100;
/// Maximum number of slot entries to keep for history
const MAX_SLOT_HISTORY: usize = 50;
/// Maximum number of transaction samples to keep
const MAX_TXN_SAMPLES: usize = 20;

/// Connection state for the proxy
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

/// Log entry with timestamp and level
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

/// Slot information with entry and transaction counts
#[derive(Debug, Clone)]
pub struct SlotInfo {
    pub slot: Slot,
    pub entry_count: u64,
    pub txn_count: u64,
    pub received_at: Instant,
    pub timestamp: DateTime<Local>,
}

/// Transaction sample for display
#[derive(Debug, Clone)]
pub struct TxnSample {
    pub slot: Slot,
    pub signature: String,
    pub received_at: DateTime<Local>,
}

/// Shred metrics for tracking proxy performance
#[derive(Debug, Default)]
pub struct ShredMetrics {
    // Current window metrics
    pub received: AtomicU64,
    pub success_forward: AtomicU64,
    pub fail_forward: AtomicU64,
    pub duplicate: AtomicU64,

    // GRPC service metrics
    pub recovered_count: AtomicU64,
    pub entry_count: AtomicU64,
    pub txn_count: AtomicU64,

    // Cumulative metrics
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

    /// Update metrics from received entry data
    pub fn add_entry(&self, entry_count: u64, txn_count: u64) {
        self.entry_count.fetch_add(entry_count, Ordering::Relaxed);
        self.txn_count.fetch_add(txn_count, Ordering::Relaxed);
        self.total_entries.fetch_add(entry_count, Ordering::Relaxed);
        self.total_txns.fetch_add(txn_count, Ordering::Relaxed);
    }

    /// Get current shreds per second rate
    pub fn get_shreds_per_sec(&self, duration_secs: f64) -> f64 {
        if duration_secs <= 0.0 {
            return 0.0;
        }
        self.received.load(Ordering::Relaxed) as f64 / duration_secs
    }

    /// Get current entries per second rate
    pub fn get_entries_per_sec(&self, duration_secs: f64) -> f64 {
        if duration_secs <= 0.0 {
            return 0.0;
        }
        self.entry_count.load(Ordering::Relaxed) as f64 / duration_secs
    }

    /// Get current transactions per second rate
    pub fn get_txns_per_sec(&self, duration_secs: f64) -> f64 {
        if duration_secs <= 0.0 {
            return 0.0;
        }
        self.txn_count.load(Ordering::Relaxed) as f64 / duration_secs
    }

    /// Reset window metrics (called periodically)
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

/// Main application state
pub struct AppState {
    // Connection info
    pub proxy_url: String,
    pub connection_state: RwLock<ConnectionState>,
    pub connected_at: RwLock<Option<Instant>>,
    pub reconnect_count: AtomicU64,

    // Metrics
    pub metrics: ShredMetrics,
    pub metrics_window_start: RwLock<Instant>,

    // Slot tracking
    pub current_slot: AtomicU64,
    pub slot_history: RwLock<VecDeque<SlotInfo>>,

    // Transaction samples
    pub txn_samples: RwLock<VecDeque<TxnSample>>,

    // Logs
    pub logs: RwLock<VecDeque<LogEntry>>,

    // UI state
    pub selected_tab: RwLock<usize>,
    pub scroll_offset: RwLock<usize>,
    pub show_help: RwLock<bool>,

    // Timing
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
            logs: RwLock::new(VecDeque::with_capacity(MAX_LOG_ENTRIES)),
            selected_tab: RwLock::new(0),
            scroll_offset: RwLock::new(0),
            show_help: RwLock::new(false),
            start_time: Instant::now(),
        }
    }

    /// Add a log entry
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

    /// Log info message
    pub fn log_info(&self, message: impl Into<String>) {
        self.log(LogLevel::Info, message);
    }

    /// Log warning message
    pub fn log_warn(&self, message: impl Into<String>) {
        self.log(LogLevel::Warn, message);
    }

    /// Log error message
    pub fn log_error(&self, message: impl Into<String>) {
        self.log(LogLevel::Error, message);
    }

    /// Log debug message
    pub fn log_debug(&self, message: impl Into<String>) {
        self.log(LogLevel::Debug, message);
    }

    /// Set connection state
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

    /// Add slot info
    pub fn add_slot(&self, slot: Slot, entry_count: u64, txn_count: u64) {
        // Update current slot
        let current = self.current_slot.load(Ordering::Relaxed);
        if slot > current {
            self.current_slot.store(slot, Ordering::Relaxed);
        }

        // Add to history
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
        });

        // Update metrics
        self.metrics.add_entry(entry_count, txn_count);
    }

    /// Add transaction sample
    pub fn add_txn_sample(&self, slot: Slot, signature: String) {
        let mut samples = self.txn_samples.write();
        if samples.len() >= MAX_TXN_SAMPLES {
            samples.pop_front();
        }
        samples.push_back(TxnSample {
            slot,
            signature,
            received_at: Local::now(),
        });
    }

    /// Get uptime duration
    pub fn uptime(&self) -> Duration {
        self.start_time.elapsed()
    }

    /// Get connection duration
    pub fn connection_duration(&self) -> Option<Duration> {
        self.connected_at.read().map(|t| t.elapsed())
    }

    /// Get metrics window duration in seconds
    pub fn metrics_window_secs(&self) -> f64 {
        self.metrics_window_start.read().elapsed().as_secs_f64()
    }

    /// Reset metrics window
    pub fn reset_metrics_window(&self) {
        *self.metrics_window_start.write() = Instant::now();
        self.metrics.reset_window();
    }

    /// Navigate to next tab
    pub fn next_tab(&self) {
        let mut tab = self.selected_tab.write();
        *tab = (*tab + 1) % 4; // 4 tabs
    }

    /// Navigate to previous tab
    pub fn prev_tab(&self) {
        let mut tab = self.selected_tab.write();
        *tab = if *tab == 0 { 3 } else { *tab - 1 };
    }

    /// Toggle help display
    pub fn toggle_help(&self) {
        let mut show = self.show_help.write();
        *show = !*show;
    }

    /// Scroll up in current view
    pub fn scroll_up(&self) {
        let mut offset = self.scroll_offset.write();
        *offset = offset.saturating_sub(1);
    }

    /// Scroll down in current view
    pub fn scroll_down(&self) {
        let mut offset = self.scroll_offset.write();
        *offset = offset.saturating_add(1);
    }
}
