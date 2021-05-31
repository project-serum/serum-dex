#!/bin/bash

set -euxo pipefail

CLUSTER=localnet
KEYPAIR_FILE=$HOME/.config/solana/id.json
CLUSTER_URL=http://localhost:8899

#
# Assumes the current working directory is top-level serum-dex dir.
#
main() {
    set +e
    #
    # Start the local validator.
    #
    solana-test-validator > validator.log &
    #
    # Wait for the validator to start.
    #
    sleep 5
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
    # Run the tests.
    #
    dex_whole_shebang
}

dex_whole_shebang() {
    #
    # Build the program.
    #
    ./do.sh build dex
    #
    # Deploy the program.
    #
    local dex_program_id="$(solana deploy --output json-compact --url ${CLUSTER_URL} dex/target/bpfel-unknown-unknown/release/serum_dex.so | jq .programId -r)"
    #
    # Run the whole-shebang.
    #
    pushd dex/crank
    cargo run -- $CLUSTER whole-shebang $KEYPAIR_FILE $dex_program_id
    popd
}

main
