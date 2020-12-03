include ../Makefile

LIB_NAME=serum_registry_program

.PHONY: test test-program deploy-lockup deploy-meta-entity deploy-all

test: deploy-lockup deploy-meta-entity deploy-super test-program
	@ # no-op

deploy-lockup:
	$(eval TMP=$(shell make -s -C ../lockup deploy))
	$(eval TEST_LOCKUP_PROGRAM_ID=$(shell echo $(TMP) | sed 's/.*{programId: \(.*\)}.*/\1/g'))

deploy-meta-entity:
	$(eval TMP=$(shell make -s -C ./meta-entity deploy))
	$(eval TEST_META_ENTITY_PROGRAM_ID=$(shell echo $(TMP) | sed 's/.*{programId: \(.*\)}.*/\1/g'))

test-program:
	make \
	TEST_PROGRAM_ID=$(TEST_PROGRAM_ID) \
	TEST_LOCKUP_PROGRAM_ID=$(TEST_LOCKUP_PROGRAM_ID) \
	TEST_META_ENTITY_PROGRAM_ID=$(TEST_META_ENTITY_PROGRAM_ID) \
	test-program-super

deploy-all: deploy-lockup deploy-meta-entity self-deploy
	echo '{ "registryProgramId": "$(TEST_PROGRAM_ID)", "lockupProgramId": "$(TEST_LOCKUP_PROGRAM_ID)", "metaEntityProgramId": "$(TEST_META_ENTITY_PROGRAM_ID)" }'
