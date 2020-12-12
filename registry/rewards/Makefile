include ../../Makefile

LIB_NAME=serum_registry_rewards_program
BPF_SDK=$(shell pwd)/../../bin/bpf-sdk

.PHONY: test deploy-dex deploy-registry test-program build-dex deploy-all

test: build-dex deploy-dex deploy-registry deploy-super test-program
	@ # no-op

build-dex:
	cd ../../ && ./do.sh build dex && cd ./registry/rewards

deploy-registry:
	$(eval TMP=$(shell make -s -C ../ deploy))
	$(eval TEST_REGISTRY_PROGRAM_ID=$(shell echo $(TMP) | sed 's/.*{programId: \(.*\)}.*/\1/g'))

deploy-dex:
	$(eval TEST_DEX_PROGRAM_ID=$(shell solana deploy $(SOL_OPTIONS) ../../dex/target/bpfel-unknown-unknown/release/serum_dex.so --use-deprecated-loader | jq .programId -r))

test-program:
	make TEST_PROGRAM_ID=$(TEST_PROGRAM_ID) TEST_DEX_PROGRAM_ID=$(TEST_DEX_PROGRAM_ID) TEST_REGISTRY_PROGRAM_ID=$(TEST_REGISTRY_PROGRAM_ID) test-program-super

deploy-all: deploy-dex self-deploy
	echo '{ "dexProgramId": "$(TEST_DEX_PROGRAM_ID)", "rewardsProgramId": "$(TEST_PROGRAM_ID)" }'
