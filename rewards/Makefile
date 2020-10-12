include ../Makefile

LIB_NAME=serum_rewards_program

.PHONY: test deploy-dex deploy-registry test-program build-dex

#
# Rewards testing requires the DEX and the Registry to be deployed in addition
# to the Rewards program, so we override the parent Makefile.
#
test: deploy-dex deploy-registry deploy-super test-program
	@ # no-op

build-dex:
	cd ../ && ./do.sh build dex && cd ./rewards

deploy-dex: build-dex
	$(eval TEST_DEX_PROGRAM_ID=$(shell solana deploy $(SOL_OPTIONS) ../dex/target/bpfel-unknown-unknown/release/serum_dex.so --use-deprecated-loader | jq .programId -r))

deploy-registry:
	$(eval TMP=$(shell make -s -C ../registry deploy))
	$(eval TEST_REGISTRY_PROGRAM_ID=$(shell echo $(TMP) | sed 's/.*{programId: \(.*\)}.*/\1/g'))

test-program:
	make TEST_PROGRAM_ID=$(TEST_PROGRAM_ID) TEST_DEX_PROGRAM_ID=$(TEST_DEX_PROGRAM_ID) TEST_REGISTRY_PROGRAM_ID=$(TEST_REGISTRY_PROGRAM_ID) test-program-super
