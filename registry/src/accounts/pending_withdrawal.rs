use borsh::{BorshDeserialize, BorshSchema, BorshSerialize};
use serum_common::pack::*;
use solana_client_gen::solana_sdk::pubkey::Pubkey;

/// PendingWithdrawal accounts are created to initiate a withdrawal.
/// Once the `end_ts` passes, the PendingWithdrawal can be burned in exchange
/// for the specified withdrawal amount.
#[derive(Debug, BorshSerialize, BorshDeserialize, BorshSchema)]
pub struct PendingWithdrawal {
    pub initialized: bool,
    pub burned: bool,
    pub member: Pubkey,
    pub start_ts: i64,
    pub end_ts: i64,
    pub amount: u64,
    pub delegate: bool,
    pub mega: bool,
}

serum_common::packable!(PendingWithdrawal);
