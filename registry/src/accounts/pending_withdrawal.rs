use serum_common::pack::*;
use solana_client_gen::solana_sdk::pubkey::Pubkey;

/// PendingWithdrawal accounts are created to initiate a withdrawal.
/// Once the timelock on the pendign withdrawal passes, the PendingWithdrawal
/// can be burned in exchange for the specified withdrawal amount.
pub struct PendingWithdrawal {
    pub initialized: bool,
    pub burned: bool,
    pub start_slot: Pubkey,
    pub amount: u64,
    pub mega_amount: u64,
    pub member: Pubkey,
}

serum_common::packable!(PendingWithdrawal);
