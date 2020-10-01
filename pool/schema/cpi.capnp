@0x80712cb6a05eec99;

using import "common.capnp".parentModule;
$parentModule("schema");

struct CpiInstr(T) {
	typeId @0 :UInt64;
	innerInstruction @1 :T;
}
