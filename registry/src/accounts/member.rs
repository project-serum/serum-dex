use serum_common::pack::*;
use solana_client_gen::solana_sdk::pubkey::Pubkey;

#[cfg(feature = "client")]
lazy_static::lazy_static! {
    pub static ref SIZE: u64 = Member::default()
                .size()
                .expect("Vesting has a fixed size");
}

/// Member account tracks membership with a node `Entity`.
#[derive(Default, Debug, serde::Serialize, serde::Deserialize)]
pub struct Member {
    /// Set by the program on creation.
    pub initialized: bool,
    /// Entity account providing membership.
    pub entity: Pubkey,
    /// The key that is allowed to redeem assets from the staking pool.
    pub beneficiary: Pubkey,
    /// Deleate key authorized to deposit or withdraw from the staking pool
    /// on behalf of the beneficiary.
    pub delegate: Pubkey,
    /// Amount of SRM staked.
    pub amount: u64,
    /// Amount of MSRM staked.
    pub mega_amount: u64,
}

serum_common::packable!(Member);
