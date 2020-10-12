use borsh::{BorshDeserialize, BorshSchema, BorshSerialize};
use serum_common::pack::*;
use solana_client_gen::solana_sdk::pubkey::Pubkey;

#[cfg(feature = "client")]
lazy_static::lazy_static! {
    pub static ref SIZE: u64 = Instance::default()
                .size()
                .expect("Instance has a fixed size");
}

#[derive(Default, Debug, BorshSerialize, BorshDeserialize, BorshSchema)]
pub struct Instance {
    pub initialized: bool,
    pub nonce: u8,
    pub registrar: Pubkey,
    pub registry_program_id: Pubkey,
    pub vault: Pubkey,
    pub dex_program_id: Pubkey,
    pub authority: Pubkey,
}

serum_common::packable!(Instance);
