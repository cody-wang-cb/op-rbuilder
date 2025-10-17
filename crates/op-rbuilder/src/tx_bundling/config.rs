//! Configuration for transaction bundling

use std::time::Duration;

/// Configuration for transaction bundling feature
#[derive(Debug, Clone)]
pub struct TxBundlingConfig {
    /// Enable transaction bundling
    pub enabled: bool,

    /// WebSocket URL to connect to for bundle data
    pub ws_url: String,

    /// Maximum number of entries to store in the bundle cache
    pub max_cache_size: Option<usize>,

    /// Reconnection delay on WebSocket disconnection
    pub reconnect_delay: Duration,

    /// Ping interval in milliseconds to keep WebSocket connection alive
    pub ping_interval_ms: u64,
}

impl Default for TxBundlingConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            ws_url: String::new(),
            max_cache_size: Some(10000),
            reconnect_delay: Duration::from_secs(5),
            ping_interval_ms: 30000, // 30 seconds
        }
    }
}

