@0x80712cb6a05eec99;

using import "common.capnp".parentModule;
$parentModule("schema");

struct CpiInstr(T) {
	typeId @0 :UInt64;
	message @1 :T;
}
