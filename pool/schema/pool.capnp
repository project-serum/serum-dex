@0x8f5ad7c43a930e75;

using import "common.capnp".parentModule;
$parentModule("schema");

struct GetBasketComponents {
	struct Request {
	}
	struct Response {
		struct BasketComponent {
			token :group {
				addr0 @0 :UInt64;
				addr1 @1 :UInt64;
				addr2 @2 :UInt64;
				addr3 @3 :UInt64;
			}
			qtyPerShare :group {
				integerPart @4 :UInt64;
				fractionalPart @5 :UInt64;
			}
		}
		basket @0 :List(BasketComponent);
	}
}

struct CreateShares {
	struct Response {
		sharesCreated @0 :UInt64;
	}
}

struct PoolRequest {
	# the unique ID of the request type
	union {
		getBasket @0 :GetBasketComponents.Request;
		createShares @1 :UInt64;
		redeemShares @2 :UInt64;
	}
}
