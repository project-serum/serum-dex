@0x8f5ad7c43a930e75;

using Common = import "common.capnp";
using Common.parentModule;

$Common.parentModule("schema");

using Cpi = import "cpi.capnp";
using Cpi.CpiInstr;
using Cpi.AccountInfo;
using Cpi.Address;

struct ProxyAccount {
	union {
		unset @0 :Void;
		proxyState @1 :ProxyState;
		# The address of an account containing a ProxyState
		# TODO support indirection to another account, for post-hoc resizing
	}
}

struct RequiredAccounts {
	# accountRefs and writableAccounts are separated to save space, and must be the same length.
	accounts @0 :List(Address);
	writableAccounts @1 :List(Bool);
}

struct BasketComponent {
	mintAddress @0 :Address;

	# binary fixed point with 64 fractional bits. Layout matches that of a
	# little-endian UInt128.
	qtyPerShare :group {
		integerPart @2 :UInt64;
		fractionalPart @1 :UInt64;
	}

	vaultAddress @3 :Address;
	vaultSignerNonce @4 :UInt8;
}

struct DelegationPolicy {
	delegateProgram @0 :Address;

	requiredCreateParams @1 :RequiredAccounts;
	requiredRedeemParams @2 :RequiredAccounts;
	requiredRefreshBasketParams @3 :RequiredAccounts;

	onlyWhenEmpty @4 :Bool;
}

struct Basket {
	union {
		# simple etf-style pools should use this option.
		static @0 :List(BasketComponent);
		dynamic :group {

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

struct PoolTokenInfo {
		mintAddress @0 :Address;
		vaultAddress @1 :Address;
		vaultSignerNonce @2 :UInt8;
}

struct ProxyState {
	basket @0 :Basket;

	poolToken @3 :PoolTokenInfo;

	# If non-null, the proxy will allow this key to change the basket.
	adminKey @1 :Address;
	# This is a safety feature to help protect the admin from accidentally
	# bricking a pool by changing the admin key to a new address.
	pendingAdminKey @2 :Address;
}

struct ProxyRequest {
	stateRoot @0 :AccountInfo;

	retbufAccount @1 :AccountInfo;
	retbufProgramId @2 :AccountInfo;

	requiredParams @3 :List(AccountInfo);

	instruction :union {
		refreshBasket @4 :Void;
		createOrRedeem @5 :CreateOrRedeemRequest;
		acceptAdmin :group {
			pendingAdminSignature @6 :AccountInfo;
		}
		adminRequest :group {
			adminSignature @7 :AccountInfo;
			union {
				setPendingAdmin @8 :Address;
				setBasket @9 :Basket;
			}
		}
		initProxy :group {
			basket @10 :Basket;
			adminKey @11 :Address;
			poolToken @12 :PoolTokenInfo;
		}
	}
}

struct CreateOrRedeemRequest {
	union {
		create :group {
			inputs @0 :List(CreationInput);
			outputTokenAccount @1 :AccountInfo;
		}
		redeem :group {
			input :group {
				tokenAccount @2 :AccountInfo;
				signerRef @3 :AccountInfo;
			}
			outputs @4 :List(RedemptionOutput);
		}
	}

	struct CreationInput {
		tokenAccount @0 :AccountInfo;
		signer @1 :AccountInfo;
		maxQtyPerShare :group {
			integerPart @3 :UInt64;
			fractionalPart @2 :UInt64;
		}
	}

	struct RedemptionOutput {
		tokenAccount @0 :AccountInfo;
		minQtyPerShare :group {
			integerPart @2 :UInt64;
			fractionalPart @1 :UInt64;
		}
	}
}

struct ProxyToDelegateRequest {
	retbufAccount @0 :AccountInfo;
	retbufProgram @1 :AccountInfo;

	requiredParams @2 :List(AccountInfo);
	union {
		getBasket @3 :Void;
		createOrRedeem @4 :CreateOrRedeemRequest;
	}

	struct GetBasketResponse {
		basket @0 :Basket;
	}
}
