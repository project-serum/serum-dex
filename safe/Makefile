#
# Path to your local solana keypair.
#
TEST_PAYER_FILEPATH="$(HOME)/.config/solana/id.json"
#
# The solana cluster to test against. Defaults to local.
#
TEST_CLUSTER_URL=localnet
#
# One can optionally set this along with the test-program command
# to avoid redeploying everytime tests are run.
#
TEST_PROGRAM_ID="8zkU53hD8dDDoBs2vEohbQTTZhpLDBwCBNCVShcYiv25"
#
# Path to the BPF sdk to build solana programs.
#
BPF_SDK=$(PWD)/../bin/bpf-sdk
#
# Parent dir for the Solana program's build target.
#
BUILD_DIR=../target/bpfel-unknown-unknown/release
#
# The program's crate name.
#
LIB_NAME=serum_safe_program

.PHONY: build deploy test test-new

build:
	$(BPF_SDK)/rust/build.sh program
	cp $(BUILD_DIR)/$(LIB_NAME).so $(BUILD_DIR)/$(LIB_NAME)_debug.so
	$(BPF_SDK)/dependencies/llvm-native/bin/llvm-objcopy --strip-all $(BUILD_DIR)/$(LIB_NAME).so $(BUILD_DIR)/$(LIB_NAME).so

deploy: build
	$(eval TEST_PROGRAM_ID=$(shell solana deploy ../target/bpfel-unknown-unknown/release/$(LIB_NAME).so | jq .programId -r))
	@echo "Deployed program at address $(TEST_PROGRAM_ID)"

test-program:
	 RUST_BACKTRACE=1 TEST_PROGRAM_ID=$(TEST_PROGRAM_ID) TEST_PAYER_FILEPATH=$(TEST_PAYER_FILEPATH) TEST_CLUSTER_URL=$(TEST_CLUSTER_URL) cargo test --features test,client,client-ext -- --nocapture $(args)

test: deploy test-program
