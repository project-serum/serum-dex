@0x80712cb6a05eec99;

using import "common.capnp".parentModule;
$parentModule("schema");

struct Address {
	word0 @0 :UInt64;
	word1 @1 :UInt64;
	word2 @2 :UInt64;
	word3 @3 :UInt64;
}

struct CpiInstr(T) {
	typeId @0 :UInt64;
	innerInstruction @1 :T;
}

struct AccountInfo {
	address @0 :Address;
}
