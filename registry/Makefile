include ../Makefile

LIB_NAME=serum_registry_program

.PHONY: test test-program deploy-lockup

#
# Registry tests need to deploy *both* the rewards and regsitry program before
# running.
#
test: deploy-lockup deploy-super test-program
	@ # no-op

deploy-lockup:
	$(eval TMP=$(shell make -s -C ../lockup deploy))
	$(eval TEST_LOCKUP_PROGRAM_ID=$(shell echo $(TMP) | sed 's/.*{programId: \(.*\)}.*/\1/g'))

test-program:
	make TEST_PROGRAM_ID=$(TEST_PROGRAM_ID) TEST_LOCKUP_PROGRAM_ID=$(TEST_LOCKUP_PROGRAM_ID) test-program-super
