#
# Path to your local solana keypair.
#
TEST_PAYER_FILEPATH="$(HOME)/.config/solana/id.json"
#
# The solana cluster to test against. Defaults to local.
#
TEST_CLUSTER=localnet
#
# The url of TEST_CLUSTER.
#
TEST_CLUSTER_URL="http://localhost:8899"
#
# One can optionally set this along with the test-program command
# to avoid redeploying everytime tests are run.
#
TEST_PROGRAM_ID=""
#
# Default options used for the solana cli.
#
SOL_OPTIONS=--url $(TEST_CLUSTER_URL) --keypair $(TEST_PAYER_FILEPATH)
#
# Path to the BPF sdk to build solana programs.
#
BPF_SDK=$(shell pwd)/../bin/bpf-sdk
#
# Parent dir for the Solana program's build target.
#
BUILD_DIR=$(shell pwd)/../target/bpfel-unknown-unknown/release
#
# The program's crate name.
#
LIB_NAME=serum_safe_program

.PHONY: build \
	build-client \
	build-program \
	deploy \
	test \
	test-program \
	test-integration \
	test-unit \
	clippy

build: build-program build-client

build-client:
ifdef features
	cargo build --features client,client-ext,$(features)
else
	cargo build --features client,client-ext
endif

workdir:
	echo $(BPF_SDK)

build-program:
	$(BPF_SDK)/rust/build.sh program
	cp $(BUILD_DIR)/$(LIB_NAME).so $(BUILD_DIR)/$(LIB_NAME)_debug.so
	$(BPF_SDK)/dependencies/llvm-native/bin/llvm-objcopy --strip-all $(BUILD_DIR)/$(LIB_NAME).so $(BUILD_DIR)/$(LIB_NAME).so

deploy: build
	$(eval TEST_PROGRAM_ID=$(shell solana deploy $(SOL_OPTIONS) $(BUILD_DIR)/$(LIB_NAME).so | jq .programId -r))
	@echo "Deployed program at address $(TEST_PROGRAM_ID)"

test-program:
	RUST_BACKTRACE=1 \
	TEST_PROGRAM_ID=$(TEST_PROGRAM_ID) \
	TEST_PAYER_FILEPATH=$(TEST_PAYER_FILEPATH) \
	TEST_CLUSTER=$(TEST_CLUSTER) \
	cargo test --features test,client,client-ext -- --nocapture $(args)

test: deploy test-program

init-test: init test

test-unit:
	cargo test --lib --features test,client,client-ext

init:
	yes | solana-keygen new --outfile $(TEST_PAYER_FILEPATH)
	yes | solana airdrop $(SOL_OPTIONS) 100

clippy:
	cargo clippy --features client,client-ext
