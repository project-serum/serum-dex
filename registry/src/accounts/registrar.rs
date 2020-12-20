use borsh::{BorshDeserialize, BorshSchema, BorshSerialize};
use serum_common::pack::*;
use solana_client_gen::solana_sdk::pubkey::Pubkey;

#[cfg(feature = "client")]
lazy_static::lazy_static! {
    pub static ref SIZE: u64 = Registrar::default()
                .size()
                .expect("Registrar has a fixed size");
}

/// Registry defines the account representing an instance of the program.
#[derive(Clone, Debug, Default, PartialEq, BorshSerialize, BorshDeserialize, BorshSchema)]
pub struct Registrar {
    /// Set by the program on initialization.
    pub initialized: bool,
    /// Priviledged account.
    pub authority: Pubkey,
    /// Nonce to derive the program-derived address owning the vaults.
    pub nonce: u8,
    /// The maximum stake per node entity, denominated in SRM.
    pub max_stake_per_entity: u64,
    /// Number of seconds that must pass for a withdrawal to complete.
    pub withdrawal_timelock: i64,
    /// Number of seconds it takes for an Entity to be "deactivated", from the
    /// moment it's MSRM amount drops below the required threshold.
    pub deactivation_timelock: i64,
    /// Global event queue for reward vendoring.
    pub reward_event_q: Pubkey,
    /// Mint of the tokens that can be staked.
    pub mint: Pubkey,
    /// Mint of the mega tokens that can be staked.
    pub mega_mint: Pubkey,
    /// Staking pool token mint.
    pub pool_mint: Pubkey,
    /// Staking pool (mega) token mint.
    pub pool_mint_mega: Pubkey,
    /// The amount of tokens (not decimal) that must be staked to get a single
    /// staking pool token.
    pub stake_rate: u64,
    /// Stake rate for the mega pool.
    pub stake_rate_mega: u64,
}

serum_common::packable!(Registrar);
