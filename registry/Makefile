include ../Makefile

LIB_NAME=serum_registry_program
#
# Add an address here and call test-program to avoid deploying everytime tests
# are run. Otherwise the tests will automatically deploy and setup the environment.
#
TEST_REWARDS_PROGRAM_ID=""

.PHONY: test test-program

#
# Registry tests need to deploy *both* the rewards and regsitry program before
# running.
#
test: deploy-rewards deploy-super test-program
	@ # no-op

deploy-rewards:
	$(eval TMP=$(shell make -s -C ../rewards deploy))
	$(eval TEST_REWARDS_PROGRAM_ID=$(shell echo $(TMP) | sed 's/.*{programId: \(.*\)}.*/\1/g'))

test-program:
	make TEST_PROGRAM_ID=$(TEST_PROGRAM_ID) TEST_REWARDS_PROGRAM_ID=$(TEST_REWARDS_PROGRAM_ID) test-program-super
