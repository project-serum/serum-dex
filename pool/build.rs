fn main() {
    ::capnpc::CompilerCommand::new()
        .file("schema/pool.capnp")
        .file("schema/pool_proxy.capnp")
        .file("schema/cpi.capnp")
        .run()
        .expect("schema compilation");
}
