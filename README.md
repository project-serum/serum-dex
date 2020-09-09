## serum-dex

# Deploying the DEX
```
# run unit tests
./do.sh test dex

# compile the dex binary
./do.sh build dex

# deploy the dex to the configured solana cluster
DEX_PROGRAM_ID="$(solana deploy dex/target/bpfel-unknown-unknown/release/serum_dex.so | jq .programId -r)"
```

# Using the client utility
```
cd crank
cargo run -- help
```
