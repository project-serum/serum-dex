pub mod cpi_capnp {
    #![allow(unused)]
    include!(concat!(env!("OUT_DIR"), "/schema/cpi_capnp.rs"));
}
pub mod pool_capnp {
    #![allow(unused)]
    include!(concat!(env!("OUT_DIR"), "/schema/pool_capnp.rs"));
}
pub mod pool_proxy_capnp {
    #![allow(unused)]
    include!(concat!(env!("OUT_DIR"), "/schema/pool_proxy_capnp.rs"));
}
