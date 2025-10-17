//! WebSocket listener for transaction bundling data

use super::store::TxBundleStore;
use crate::metrics::OpRBuilderMetrics;
use alloy_primitives::{Bytes, B256};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use std::{sync::Arc, time::Duration};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, error, info, warn};

/// Message format from the WebSocket server
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct BundleMessage {
    #[serde(rename = "type")]
    type_field: String,
    #[serde(default)]
    counter: Option<u64>,
    timestamp: String,
    data: Vec<String>,
}

/// Spawn a background task that listens to the bundle WebSocket stream
pub fn spawn_bundle_listener(
    ws_url: String,
    store: Arc<TxBundleStore>,
    reconnect_delay: Duration,
    ping_interval_ms: u64,
    metrics: Arc<OpRBuilderMetrics>,
) -> tokio::task::JoinHandle<()> {
    info!(
        target: "tx_bundling",
        ws_url = %ws_url,
        reconnect_delay_secs = reconnect_delay.as_secs(),
        ping_interval_ms = ping_interval_ms,
        "Spawning transaction bundle listener task"
    );

    tokio::spawn(async move {
        info!(
            target: "tx_bundling",
            ws_url = %ws_url,
            "Transaction bundle listener task started, entering connection loop"
        );

        loop {
            match connect_and_listen(&ws_url, store.clone(), ping_interval_ms, metrics.clone()).await {
                Ok(_) => {
                    info!(
                        target: "tx_bundling",
                        "Bundle stream connection closed normally"
                    );
                }
                Err(e) => {
                    error!(
                        target: "tx_bundling",
                        error = %e,
                        reconnect_delay_secs = reconnect_delay.as_secs(),
                        "Bundle stream error, reconnecting..."
                    );
                }
            }

            // Wait before reconnecting
            tokio::time::sleep(reconnect_delay).await;
        }
    })
}

/// Connect to WebSocket and listen for bundle messages
async fn connect_and_listen(
    ws_url: &str,
    store: Arc<TxBundleStore>,
    ping_interval_ms: u64,
    metrics: Arc<OpRBuilderMetrics>,
) -> eyre::Result<()> {
    info!(
        target: "tx_bundling",
        ws_url = %ws_url,
        ping_interval_ms = ping_interval_ms,
        "Attempting to connect to bundle stream..."
    );

    let (ws_stream, response) = connect_async(ws_url).await?;
    
    info!(
        target: "tx_bundling",
        status = ?response.status(),
        "Successfully connected to bundle stream"
    );

    let (mut write, mut read) = ws_stream.split();

    // Send ping periodically to keep connection alive
    let ping_handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(ping_interval_ms));
        loop {
            interval.tick().await;
            if write.send(Message::Ping(vec![].into())).await.is_err() {
                break;
            }
        }
    });

    info!(
        target: "tx_bundling",
        "Bundle listener ready, waiting for messages..."
    );

    while let Some(msg) = read.next().await {
        let msg = msg?;

        match msg {
            Message::Text(text) => {
                metrics.bundle_listener_messages_received.increment(1);

                match process_bundle_message(&text, &store) {
                    Ok(true) => {
                        metrics.bundle_listener_bundles_stored.increment(1);
                        debug!(
                            target: "tx_bundling",
                            cache_size = store.len(),
                            "Bundle stored successfully"
                        );
                    }
                    Ok(false) => {
                        // Message was not a valid bundle, skip
                        debug!(
                            target: "tx_bundling",
                            "Received non-bundle message, skipping"
                        );
                    }
                    Err(e) => {
                        warn!(
                            target: "tx_bundling",
                            error = %e,
                            "Failed to process bundle message"
                        );
                    }
                }
            }
            Message::Ping(_data) => {
                debug!(target: "tx_bundling", "Received ping, sending pong");
                // Pong is handled automatically by tungstenite
            }
            Message::Pong(_) => {
                debug!(target: "tx_bundling", "Received pong");
            }
            Message::Close(frame) => {
                info!(
                    target: "tx_bundling",
                    ?frame,
                    "Received close frame from server"
                );
                break;
            }
            _ => {}
        }
    }

    ping_handle.abort();

    info!(
        target: "tx_bundling",
        "Bundle listener disconnected"
    );

    Ok(())
}

/// Process a single bundle message and store it if valid
fn process_bundle_message(text: &str, store: &TxBundleStore) -> eyre::Result<bool> {
    let bundle_msg: BundleMessage = serde_json::from_str(text)?;

    // Accept both "data_stream" and "transaction_processed" message types
    let is_valid_type = bundle_msg.type_field == "data_stream" 
        || bundle_msg.type_field == "transaction_processed";
    
    if !is_valid_type || bundle_msg.data.len() != 2 {
        return Ok(false);
    }

    // Parse tx_hash (data[0]) and bundled_tx_data (data[1])
    let tx_hash: B256 = bundle_msg.data[0].parse()?;
    let bundled_tx_data: Bytes = bundle_msg.data[1].parse()?;

    // Get size before moving
    let bundled_tx_size = bundled_tx_data.len();

    // Store the mapping
    store.insert(tx_hash, bundled_tx_data);

    debug!(
        target: "tx_bundling",
        tx_hash = ?tx_hash,
        bundled_tx_size,
        counter = ?bundle_msg.counter,
        message_type = %bundle_msg.type_field,
        "Stored bundled transaction"
    );

    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_bundle_message() {
        let store = Arc::new(TxBundleStore::new(None));
        
        let json = r#"{
            "type": "data_stream",
            "counter": 5,
            "timestamp": "2025-10-14T18:42:45.023Z",
            "data": [
                "0x2d41fd9ae1882c7a68de4e260fda998856e64fab528cefa5e0ebbe0aa2ca8bbb",
                "0x02f8"
            ]
        }"#;

        let result = process_bundle_message(json, &store);
        assert!(result.is_ok());
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn test_process_invalid_message() {
        let store = Arc::new(TxBundleStore::new(None));
        
        let json = r#"{
            "type": "other_type",
            "data": ["0xabc"]
        }"#;

        let result = process_bundle_message(json, &store);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), false);
        assert_eq!(store.len(), 0);
    }
}

