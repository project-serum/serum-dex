use serum_pool::{declare_pool_entrypoint, Pool};

enum SimplePool {}

impl Pool for SimplePool {}

#[cfg(not(feature = "no-entrypoint"))]
declare_pool_entrypoint!(SimplePool);
