# This is the parent Makefile used by Solana program crates. It's expected
# this is included in child Makefiles with commands overriden as desired
# (this is why all the targets here end with % wildcards). In addition to
# override targets, one can customize the behavior, by override following
# variables in a child Makefile. See `lockup/Makefile` for an example of
# a child.

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
# The name of the directory holding your Solana program, relative to the Makefile.
#
PROGRAM_DIRNAME=program
#
# Parent dir for the Solana program's build target.
#
BUILD_DIR=$(shell pwd)/$(PROGRAM_DIRNAME)/target/bpfel-unknown-unknown/release
#
# The program's crate name.
#
LIB_NAME=<your-solana-program>

.PHONY: buil% \
	build-clien% \
	build-progra% \
	deplo% \
	tes% \
	test-progra% \
	test-integratio% \
	test-uni% \
	clipp% \
	custo%

buil%: build-progra% build-clien%
	@ # no-op

build-clien%:
ifdef features
	@cargo build --features client,$(features)
else
	@cargo build --features client
endif

build-progra%:
	@$(BPF_SDK)/rust/build.sh $(PROGRAM_DIRNAME)
	@cp $(BUILD_DIR)/$(LIB_NAME).so $(BUILD_DIR)/$(LIB_NAME)_debug.so
	@$(BPF_SDK)/dependencies/llvm-native/bin/llvm-objcopy --strip-all $(BUILD_DIR)/$(LIB_NAME).so $(BUILD_DIR)/$(LIB_NAME).so

deplo%: buil%
	@$(eval TEST_PROGRAM_ID=$(shell solana deploy $(SOL_OPTIONS) $(BUILD_DIR)/$(LIB_NAME).so | jq .programId -r))
	@echo "{\"programId\": \"$(TEST_PROGRAM_ID)\"}"

test-progra%:
	RUST_BACKTRACE=1 \
	TEST_PROGRAM_ID=$(TEST_PROGRAM_ID) \
	TEST_PAYER_FILEPATH=$(TEST_PAYER_FILEPATH) \
	TEST_CLUSTER=$(TEST_CLUSTER) \
	TEST_WHITELIST_PROGRAM_ID=$(TEST_WHITELIST_PROGRAM_ID) \
	TEST_DEX_PROGRAM_ID=$(TEST_DEX_PROGRAM_ID) \
	TEST_REGISTRY_PROGRAM_ID=$(TEST_REGISTRY_PROGRAM_ID) \
	TEST_LOCKUP_PROGRAM_ID=$(TEST_LOCKUP_PROGRAM_ID) \
	cargo test --features test,client -- --nocapture $(args)

tes%: deplo% test-progra%
	@ # no-op

init-tes%:
	@make init
	@make test

test-uni%:
	@RUST_BACKTRACE=1 \
	cargo test --lib --features test,client -- --nocapture $(args)

ini%:
	@yes | solana-keygen new --outfile $(TEST_PAYER_FILEPATH)
	@yes | solana airdrop $(SOL_OPTIONS) 100

clipp%:
	@cargo clippy --features client
