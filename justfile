# Build and run op-rbuilder in playground mode for testing with tx bundling
run-playground:
  cargo build --bin op-rbuilder -p op-rbuilder
  ./target/debug/op-rbuilder node --builder.playground --datadir ~/.playground/devnet/rbuilder \
    --flashblocks.enabled \
    --tx-bundling.enabled \
    --tx-bundling.ws-url ws://localhost:8080 \
    --tx-bundling.ping-interval-ms 30000 \
    --tx-bundling.max-cache-size 10000 \
    --tx-bundling.reconnect-delay-secs 5 

# Run the complete test suite (genesis generation, build, and tests)
run-tests:
  just generate-test-genesis
  just build-op-rbuilder
  just run-tests-op-rbuilder

# Download `op-reth` binary
download-op-reth:
  ./scripts/ci/download-op-reth.sh

# Generate a genesis file (for tests)
generate-test-genesis:
  cargo run -p op-rbuilder --features="testing" --bin tester -- genesis --output genesis.json


# Build the op-rbuilder binary
build-op-rbuilder:
  cargo build -p op-rbuilder --bin op-rbuilder

# Run the integration tests
run-tests-op-rbuilder:
  PATH=$PATH:$(pwd) cargo test --package op-rbuilder --lib
