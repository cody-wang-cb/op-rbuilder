//! Transaction bundling support for op-rbuilder
//!
//! This module provides functionality to listen to external WebSocket streams
//! for additional transactions that should be bundled with existing transactions
//! in flashblocks.

pub mod config;
pub mod listener;
pub mod store;

pub use config::TxBundlingConfig;
pub use listener::spawn_bundle_listener;
pub use store::TxBundleStore;

