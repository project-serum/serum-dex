use borsh::{BorshDeserialize, BorshSchema, BorshSerialize};
use serum_common::pack::Pack;
use serum_common::pack::*;
use solana_client_gen::solana_sdk::pubkey::Pubkey;

#[cfg(feature = "client")]
lazy_static::lazy_static! {
    pub static ref SIZE: u64 = PendingWithdrawal::default()
                .size()
                .expect("Vesting has a fixed size");
}

#[derive(Debug, Default, BorshSerialize, BorshDeserialize, BorshSchema)]
pub struct PendingWithdrawal {
    /// Set by the program on initialization.
    pub initialized: bool,
    /// Member this account belongs to.
    pub member: Pubkey,
    /// One time token. True if the withdrawal has been completed.
    pub burned: bool,
    /// The pool being withdrawn from.
    pub pool: Pubkey,
    /// Unix timestamp when this account was initialized.
    pub start_ts: i64,
    /// Timestamp when the pending withdrawal completes.
    pub end_ts: i64,
    /// The number of tokens redeemed from the staking pool.
    pub amount: u64,
    /// The Member account's set of vaults this withdrawal belongs to.
    pub balance_id: Pubkey,
}

serum_common::packable!(PendingWithdrawal);
