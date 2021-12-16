#!/bin/bash

set -euxo pipefail

CLUSTER=localnet
KEYPAIR_FILE=$HOME/.config/solana/id.json
CLUSTER_URL=http://localhost:8899
PROGRAM_ID="2SXFv8tTmavm8uSAg3ft1JjttzJvgwXZiUPa9xuUbqH2"

#
# Assumes the current working directory is top-level serum-dex dir.
#
main() {
    set +e
    #
    # Build the program.
    #
    cd ./dex && cargo build-bpf && cd ../
    #
    # Start the local validator.
    #
    solana-test-validator --bpf-program $PROGRAM_ID dex/target/deploy/serum_dex.so > validator.log &
    #
    # Wait for the validator to start.
    #
    sleep 5
    #
    # Run the whole-shebang.
    #
    pushd dex/crank
    cargo run -- $CLUSTER whole-shebang $KEYPAIR_FILE $PROGRAM_ID
    popd
    #
    # Create a keypair for the tests.
    #
    yes | solana-keygen new --outfile $KEYPAIR_FILE
    #
    # Fund the keypair.
    #
    yes | solana airdrop --url $CLUSTER_URL 100
    set -e
    #
    # Run the unit tests.
    #
    pushd dex
    cargo test
    popd
}

main
