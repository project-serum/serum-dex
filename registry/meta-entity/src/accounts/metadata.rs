use borsh::{BorshDeserialize, BorshSerialize};
use serum_common::pack::*;
use solana_client_gen::solana_sdk::pubkey::Pubkey;

#[derive(Debug, Default, BorshSerialize, BorshDeserialize)]
pub struct Metadata {
    pub initialized: bool,
    pub entity: Pubkey,
    pub authority: Pubkey,
    pub name: String,
    pub about: String,
    pub image_url: String,
    pub chat: Pubkey,
}

impl Metadata {
    pub fn size() -> u64 {
        280 * 2 + 32
    }
}

serum_common::packable!(Metadata);
