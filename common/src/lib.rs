#![cfg_attr(feature = "strict", deny(warnings))]

#[cfg(feature = "client")]
pub mod client;
#[macro_use]
pub mod pack;
pub mod accounts;
#[cfg(feature = "program")]
pub mod program;

// TODO: Import the shared_mem crate instead of hardcoding here.
//       The shared memory program is awaiting audit and so is not deployed
//       to mainnet, yet.
pub mod shared_mem {
    #[cfg(not(feature = "devnet"))]
    solana_sdk::declare_id!("shmem4EWT2sPdVGvTZCzXXRAURL9G5vpPxNwSeKhHUL");
    #[cfg(feature = "devnet")]
    solana_sdk::declare_id!("3w2Q6XjS2BDpxHVRzs8oWbNuH7ivZp1mVo3mbq318oyG");
}
