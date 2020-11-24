use serum_pool::{declare_pool_entrypoint, Pool};

enum SimplePool {}

impl Pool for SimplePool {}

#[cfg(feature = "program")]
declare_pool_entrypoint!(SimplePool);
