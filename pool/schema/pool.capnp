@0x8f5ad7c43a930e75;

using Common = import "common.capnp";
using Common.parentModule;

$Common.parentModule("schema");
using import "cpi.capnp".CpiInstr;

struct Address {
		word0 @0 :UInt64;
		word1 @1 :UInt64;
		word2 @2 :UInt64;
		word3 @3 :UInt64;
}

struct ResizableProxyState {
	union {
		inPlace @0 :ProxyState;
		# The address of an account containing a ProxyState
		indirect @1 :Address;
	}
}

struct RequiredAccounts {
	# compact list of references into a separate list of accounts, with writablility requirements.
	# accountRefs and writableAccounts are separated to save space, and must be the same length.
	accountRefs @0 :List(UInt8);
	writableAccounts @1 :List(Bool);
}

struct BasketComponent {
	# the index, in addressTable, of the mint address for this component
	mintAddressRef @0 :UInt8;

	# binary fixed point with 64 fractional bits. Layout matches that of a
	# little-endian UInt128.
	qtyPerShare :group {
		integerPart @2 :UInt64;
		fractionalPart @1 :UInt64;
	}

	vaultAddressRef @3 :UInt8;
	vaultSignerNonce @4 :UInt8;
}

struct DelegationPolicy {
	delegateProgramRef @0 :UInt8;
	requiredAccountRefs @1 :List(UInt8);

	createParams @2 :RequiredAccounts;
	redeemParams @3 :RequiredAccounts;
	refreshBasketParams @4 :RequiredAccounts;

	onlyWhenEmpty @5 :Bool;
}

struct Basket {
	union {
		# simple etf-style pools should use this option.
		# components must be sorted lexicographically by index
		static @0 :List(BasketComponent);
		dynamic :group {

			# components must be sorted lexicographically by mint address
			basket :group {
				creationBasket @1 :List(BasketComponent);
				redemptionBasket @2 :List(BasketComponent);
			}

			# Pools may keep most of their assets locked up somewhere,
			# with some held in reserve to service redemptions.
			#
			# For example, an AMM on Serum may keep most of its assets on
			# open orders, and thus need to wait for cancels to be cranked
			# before it can process large redemptions.
			#
			# If delegate is not null, the proxy will call the delegate program
			# to process any creations or redemptions. In this case, creationBasket
			# and redemptionBasket are purely indicative, for the benefit of GUIs.
			delegation @3 :DelegationPolicy;
		}
	}
}

struct ProxyState {
	# dedup table for addresses. Must be sorted lexicographically by address.
	addressTable @0 :List(Address);

	basket @1 :Basket;

	# If non-null, the proxy will allow this key to change the basket.
	adminKey @2 :Address;
	# This is a safety feature to help protect the admin from accidentally
	# bricking a pool by changing the admin key to a new address.
	pendingAdminKey @3 :Address;

}

struct ProxyRequest {
	stateAccountsRefs :union {
		root @18 :UInt8;
		indirect :union {
			none @19 :Void;
			some @20 :UInt8;
		}
	}

	retbufAccountRef @0 :UInt8;
	retbufProgramIdRef @1 :UInt8;

	requiredParamsRange :group {
		beginRef @2 :UInt8;
		count @3 :UInt8;
	}

	instruction :union {
		refreshBasket @4 :Void;
		createShares :group {
			inputTokenAccountsRange :group {
				beginRef @5 :UInt8;
				count @6 :UInt8;
			}
			outputTokenAccountRef @7 :UInt8;
		}
		redeemShares :group {
			inputTokenAccountRef @8 :UInt8;
			outputTokenAccountsRange :group {
				beginRef @9 :UInt8;
				count @10 :UInt8;
			}
		}
		acceptAdmin :group {
			pendingAdminSignatureRef @11 :UInt8;
		}
		adminRequest :union {
			adminSignatureRef @12 :UInt8;
			setPendingAdmin :group {
				newAdmin @13 :Address;
			}
			setBasket :group {
				addressTable @14 :List(Address);
				basket @15 :Basket;
			}
			setDelegationPolicy :group {
				addressTable @16 :List(Address);
				delegation @17 :DelegationPolicy;
			}
		}
	}
}

# The ref fields are offsets into the &[AccountInfo] accompanying the request.
# The ranges may overlap.
struct ProxyToDelegateRequest {
	retbufAccountRef @0 :UInt8;
	retbufProgramIdRef @1 :UInt8;

	requiredParamsRange :group {
		beginRef @2 :UInt8;
		count @3 :UInt8;
	}
	union {
		getBasket @4 :Void;
		createShares :group {
			inputTokenAccountsRange :group {
				beginRef @5 :UInt8;
				count @6 :UInt8;
			}
			outputTokenAccountRef @7 :UInt8;
		}
		redeemShares :group {
			inputTokenAccountRef @8 :UInt8;
			outputTokenAccountsRange :group {
				beginRef @9 :UInt8;
				count @10 :UInt8;
			}
		}
	}

	struct GetBasketResponse {
		# dedup table for addresses. Must be sorted lexicographically by address.
		addressTable @0 :List(Address);
		basket @1 :Basket;
	}
}
