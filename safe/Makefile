TEST_PROGRAM_ID=GQGW2mHzChrRghEL6PngZpR77jXfbDJxEUGKu8B4UzGi
TEST_PAYER_FILEPATH="/home/armaniferrante/.config/solana/id.json"
TEST_CLUSTER_URL=l

default:
	# no-op

test:
	 RUST_BACKTRACE=1 TEST_PROGRAM_ID=$(TEST_PROGRAM_ID) TEST_PAYER_FILEPATH=$(TEST_PAYER_FILEPATH) TEST_CLUSTER_URL=$(TEST_CLUSTER_URL) cargo test --features test,client -- --nocapture

build:
	$(MAKE) -C program build

deploy:
	cd program && $(MAKE) deploy && cd ..
