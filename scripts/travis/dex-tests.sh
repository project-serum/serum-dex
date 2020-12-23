#!/bin/bash

set -euxo pipefail

CLUSTER=localnet
KEYPAIR_FILE=$HOME/.config/solana/id.json
CLUSTER_URL=http://localhost:8899

#
# Assumes the current working directory is top-level serum-dex dir.
#
main() {
    #
    # Create a keypair for the tests.
    #
    set +e
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
    local dex_program_id="$(solana deploy --url ${CLUSTER_URL} dex/target/bpfel-unknown-unknown/release/serum_dex.so --use-deprecated-loader | jq .programId -r)"
    #
    # Run the whole-shebang.
    #
    pushd dex/crank
    cargo run -- $CLUSTER whole-shebang $KEYPAIR_FILE $dex_program_id
    popd
}

main
