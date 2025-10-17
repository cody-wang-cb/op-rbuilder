# Transaction Bundling Implementation

## Overview

This document describes the transaction bundling feature added to op-rbuilder. This feature allows the builder to include additional transactions from an external WebSocket stream at the end of each flashblock.

## Architecture

### Components

1. **WebSocket Listener** (`src/tx_bundling/listener.rs`)

    - Connects to external WebSocket server
    - Listens for bundled transaction data
    - Auto-reconnects on disconnection
    - Parses JSON messages in the format:
        ```json
        {
            "type": "data_stream",
            "counter": 5,
            "timestamp": "2025-10-14T18:42:45.023Z",
            "data": ["0x<tx_hash>", "0x<raw_tx_data>"]
        }
        ```

2. **Bundle Store** (`src/tx_bundling/store.rs`)

    - Thread-safe `DashMap<B256, Bytes>` storage
    - Maps original transaction hash → bundled transaction data
    - LRU-like eviction when cache exceeds max size
    - Configurable max cache size

3. **Configuration** (`src/tx_bundling/config.rs`)

    - `TxBundlingConfig` structure
    - CLI arguments integration
    - Environment variable support

4. **Execution Integration** (`src/builders/flashblocks/payload.rs`)

    - `try_execute_bundled_transactions()` method
    - Called at end of each flashblock building
    - Checks each executed transaction for bundled data
    - Executes bundled transactions sequentially
    - Validates gas and DA limits
    - Logs failures but continues flashblock building

5. **Helper Method** (`src/builders/context.rs`)
    - `execute_single_transaction()` helper
    - Reusable transaction execution logic
    - Used by bundled tx, builder tx, and potentially others

## Configuration

### CLI Arguments

```bash
--tx-bundling.enabled              # Enable/disable bundling (default: false)
--tx-bundling.ws-url <URL>         # WebSocket URL to connect to
--tx-bundling.max-cache-size <N>   # Max cache entries (default: 10000)
--tx-bundling.reconnect-delay-secs # Reconnect delay in seconds (default: 5)
```

### Environment Variables

```bash
TX_BUNDLING_ENABLED=true
TX_BUNDLING_WS_URL=ws://localhost:8080/bundle-stream
TX_BUNDLING_MAX_CACHE_SIZE=10000
TX_BUNDLING_RECONNECT_DELAY_SECS=5
```

## Execution Flow

### Flashblock Building

1. **execute_best_transactions()** - Execute mempool transactions
2. **try_execute_bundled_transactions()** ← NEW
    - Check each executed transaction for bundled data
    - Decode bundled transaction from raw bytes
    - Validate gas and DA limits
    - Execute using `execute_single_transaction()` helper
    - Update info with new transaction if successful
    - Continue to next transaction on failure
3. **add_builder_txs()** - Add builder transactions
4. **build_block()** - Finalize the flashblock

### Transaction Execution

```rust
// For each executed transaction in current flashblock:
if let Some(bundled_tx_data) = store.get(&tx_hash) {
    // 1. Decode transaction
    let tx = OpTransactionSigned::decode(bundled_tx_data)?;

    // 2. Validate limits
    check_gas_limit(tx, remaining_gas)?;
    check_da_limit(tx, remaining_da)?;

    // 3. Execute transaction
    let (gas_used, da_size) = execute_single_transaction(tx)?;

    // 4. Update tracking
    remaining_gas -= gas_used;
    remaining_da -= da_size;
}
```

## Metrics

New metrics added for observability:

-   `bundled_tx_attempts_total` - Counter of bundled tx execution attempts
-   `bundled_tx_success_total` - Counter of successful bundled txs
-   `bundled_tx_failures_total` - Counter of failed bundled txs
-   `bundled_tx_gas_used` - Histogram of gas used by bundled txs
-   `bundle_cache_size` - Gauge of current cache size
-   `bundled_tx_execution_duration` - Histogram of execution duration

## Key Design Decisions

### 1. Timing: End of Flashblock

Bundled transactions are executed **after** mempool transactions because:

-   Bundled data arrives asynchronously via WebSocket
-   May not be available when original transaction executes
-   End of flashblock ensures maximum opportunity for data to arrive

### 2. Storage: DashMap

Using `DashMap` for lock-free concurrent access:

-   WebSocket listener writes
-   Multiple flashblock builders read
-   No blocking between operations

### 3. Execution: Helper Method

Created reusable `execute_single_transaction()` helper:

-   Encapsulates common execution pattern
-   Used for bundled txs, builder txs, etc.
-   Single source of truth for execution logic
-   Easier testing and maintenance

### 4. Error Handling: Continue on Failure

Failed bundled transactions don't fail the flashblock:

-   Log error with context
-   Increment failure metrics
-   Continue with remaining transactions
-   Original transaction always stays in flashblock

### 5. Resource Management

Explicit gas and DA limit tracking:

-   Pre-check before execution
-   Account for consumed resources
-   Prevent exceeding flashblock limits

## Usage Example

### Starting the Builder

```bash
cargo run -p op-rbuilder --bin op-rbuilder -- node \
    --chain /path/to/chain-config.json \
    --http \
    --authrpc.port 9551 \
    --authrpc.jwtsecret /path/to/jwt.hex \
    --flashblocks.enabled \
    --flashblocks.port 1111 \
    --tx-bundling.enabled \
    --tx-bundling.ws-url ws://localhost:8080/bundle-stream
```

### WebSocket Server Message Format

The external WebSocket server should send messages in this format:

```json
{
    "type": "data_stream",
    "counter": 5,
    "timestamp": "2025-10-14T18:42:45.023Z",
    "data": [
        "0x2d41fd9ae1882c7a68de4e260fda998856e64fab528cefa5e0ebbe0aa2ca8bbb",
        "0x7ef90104a0268b9de5a330d16bfa99d4ec0513d56d6db42d1effb3b694a7213856ac2987bb94..."
    ]
}
```

Where:

-   `data[0]` is the original transaction hash (hex string with 0x prefix)
-   `data[1]` is the bundled transaction's RLP-encoded bytes (hex string with 0x prefix)

## Files Modified/Created

### New Files

-   `src/tx_bundling/mod.rs` - Module exports
-   `src/tx_bundling/store.rs` - Bundle storage implementation
-   `src/tx_bundling/listener.rs` - WebSocket listener
-   `src/tx_bundling/config.rs` - Configuration structures

### Modified Files

-   `src/args/op.rs` - Added CLI arguments
-   `src/builders/flashblocks/config.rs` - Added tx bundling config
-   `src/builders/flashblocks/payload.rs` - Added bundled tx execution
-   `src/builders/flashblocks/service.rs` - Wire up listener and store
-   `src/builders/context.rs` - Added helper method
-   `src/metrics.rs` - Added bundled tx metrics
-   `src/lib.rs` - Exported tx_bundling module

## Testing

The implementation includes:

-   Unit tests for bundle store operations
-   Unit tests for WebSocket message parsing
-   Integration with existing flashblocks tests

To run tests:

```bash
cargo test --package op-rbuilder
```

## Future Enhancements

Potential improvements:

1. **Batch Execution**: Execute multiple bundled txs in one EVM call
2. **Ordering**: Allow bundled txs to specify execution order
3. **Conditional Execution**: Support conditional bundled txs
4. **TTL**: Add time-to-live for cached bundle data
5. **Compression**: Support compressed bundle data
6. **Multiple Sources**: Support multiple WebSocket sources

## Security Considerations

1. **Validation**: All bundled transactions are fully validated
2. **Limits**: Gas and DA limits strictly enforced
3. **Isolation**: Failed bundled txs don't affect flashblock
4. **DoS Protection**: Cache size limits prevent memory exhaustion
5. **Revert Protection**: Bundled txs that revert are rejected

## Performance Impact

-   **Storage**: O(1) lookup in DashMap
-   **Execution**: Sequential, one bundled tx per original tx
-   **Network**: Minimal overhead, WebSocket listener is async
-   **Memory**: Bounded by max_cache_size configuration

## Troubleshooting

### Bundled transactions not executing

1. Check WebSocket connection:

    ```bash
    # Look for connection logs
    grep "tx_bundling" logs/op-rbuilder.log
    ```

2. Verify bundle data format:

    ```bash
    # Check for parsing errors
    grep "Failed to process bundle message" logs/op-rbuilder.log
    ```

3. Check metrics:
    ```bash
    curl http://localhost:9011/metrics | grep bundled_tx
    ```

### High failure rate

1. Check gas limits
2. Check DA size limits
3. Verify transaction encoding
4. Check for nonce issues

## Conclusion

The transaction bundling feature provides a flexible mechanism for including additional transactions in flashblocks based on external data sources. The implementation is robust, observable, and maintains the integrity of flashblock building even when bundled transactions fail.
