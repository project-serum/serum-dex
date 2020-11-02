#![cfg_attr(feature = "strict", deny(warnings))]
#![allow(dead_code)]

use borsh::{BorshDeserialize, BorshSchema, BorshSerialize};
use serum_common::pack::*;
use solana_client_gen::prelude::*;

pub mod accounts;
pub mod error;

#[cfg_attr(feature = "client", solana_client_gen)]
pub mod instruction {
    use super::*;
    #[derive(Debug, BorshSerialize, BorshDeserialize, BorshSchema)]
    pub enum MetaEntityInstruction {
        Initialize {
            entity: Pubkey,
            authority: Pubkey,
            name: String,
            about: String,
            image_url: String,
            chat: Pubkey,
        },
        Update {
            name: Option<String>,
            about: Option<String>,
            image_url: Option<String>,
            chat: Option<Pubkey>,
        },
        SendMessage {
            data: Vec<u8>,
        },
    }
}

serum_common::packable!(instruction::MetaEntityInstruction);
