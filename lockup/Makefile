include ../Makefile

LIB_NAME=serum_lockup_program

.PHONY: test test-program

test: deploy-super test-program
	@ # no-op

test-program:
	make TEST_PROGRAM_ID=$(TEST_PROGRAM_ID) test-program-super
