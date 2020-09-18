TEST_PROGRAM_ID=GPHmuuGykRPqWSni5ZJY52wJZdMWTe7sDTcfNfUJZyqJ
TEST_PAYER_FILEPATH="/home/armaniferrante/.config/solana/id.json"
TEST_CLUSTER_URL=l

default:
	# no-op

test:
	 RUST_BACKTRACE=1 TEST_PROGRAM_ID=$(TEST_PROGRAM_ID) TEST_PAYER_FILEPATH=$(TEST_PAYER_FILEPATH) TEST_CLUSTER_URL=$(TEST_CLUSTER_URL) cargo test --features test,client -- --nocapture

build:
	cd program && $(MAKE) build && cd ..

deploy:
	cd program && $(MAKE) deploy && cd ..
