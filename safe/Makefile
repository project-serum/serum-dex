TEST_PROGRAM_ID=9EEM2Bx9hZr8H3TqX1ic38U8SkatQroFHYX3Wcu8ZLCL
TEST_PAYER_FILEPATH="/home/armaniferrante/.config/solana/id.json"
TEST_CLUSTER_URL=l

default:
	# no-op

test:
	 TEST_PROGRAM_ID=$(TEST_PROGRAM_ID) TEST_PAYER_FILEPATH=$(TEST_PAYER_FILEPATH) TEST_CLUSTER_URL=$(TEST_CLUSTER_URL) cargo test --features test,client -- --nocapture

build:
	$(MAKE) -C program build

deploy:
	cd program && $(MAKE) deploy && cd ..
