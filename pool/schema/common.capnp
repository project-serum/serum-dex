@0xf4689023b21552fe;

# indicates the parent module for rust codegen.
# included here so schema users don't depend on the rust schema compiler
annotation parentModule @0xabee386cd1450364 (file) :Text;

# An Solana address
struct Address {
	word0 @0 :UInt64;
	word1 @1 :UInt64;
	word2 @2 :UInt64;
	word3 @3 :UInt64;
}
