use crate::error::RegistryError;
use borsh::{BorshDeserialize, BorshSerialize};
use serum_common::pack::*;
use solana_client_gen::solana_sdk::account_info::AccountInfo;
use solana_client_gen::solana_sdk::pubkey::Pubkey;

#[derive(Default, Debug, BorshSerialize, BorshDeserialize)]
pub struct LockedRewardVendor {
    pub initialized: bool,
    pub registrar: Pubkey,
    pub vault: Pubkey,
    pub nonce: u8,
    pub pool_token_supply: u64,
    // The position of the reward event associated with this vendor.
    // Used to perform access control on member accounts attempting
    // to claim the reward. Reject any member who's cursor is greater
    // than this cursor.
    pub reward_event_q_cursor: u32,
    pub start_ts: i64,
    pub end_ts: i64,
    pub expiry_ts: i64,
    pub expiry_receiver: Pubkey,
    pub total: u64,
    pub period_count: u64,
}

impl LockedRewardVendor {
    pub fn initialized(account_info: &AccountInfo) -> Result<bool, RegistryError> {
        let r = match account_info.try_borrow_data()?[0] {
            1 => true,
            _ => false,
        };
        Ok(r)
    }
}

serum_common::packable!(LockedRewardVendor);
