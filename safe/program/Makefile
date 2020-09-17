BPF_SDK=$(PWD)/../../bin/bpf-sdk

BUILD_DIR=../../target/bpfel-unknown-unknown/release
LIB_NAME=serum_safe_program

.PHONY: build deploy clean clippy fmt test

default: build

build:
	$(BPF_SDK)/rust/build.sh .
	cp $(BUILD_DIR)/$(LIB_NAME).so $(BUILD_DIR)/$(LIB_NAME)_debug.so
	$(BPF_SDK)/dependencies/llvm-native/bin/llvm-objcopy --strip-all $(BUILD_DIR)/$(LIB_NAME).so $(BUILD_DIR)/$(LIB_NAME).so

deploy: build
	solana deploy ../../target/bpfel-unknown-unknown/release/$(LIB_NAME).so

clean:
	rm -rf target

clippy:
	cargo clippy --features=program

fmt:
	cargo fmt

test:
	cargo test --features=program,test
