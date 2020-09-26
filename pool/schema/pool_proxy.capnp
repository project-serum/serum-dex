@0xc56960dbb4ba2ab8;

using import "common.capnp".parentModule;
$parentModule("schema");

using Pool = import "pool.capnp";

struct ProxyRequest {
	union {
		initProxy @0 :Void;
		poolRequest @1 :Pool.PoolRequest;
	}
}
