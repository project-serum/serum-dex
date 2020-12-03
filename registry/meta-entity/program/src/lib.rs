//! Program entrypoint.

#![cfg_attr(feature = "strict", deny(warnings))]

use serum_common::pack::Pack;
use serum_meta_entity::error::{MetaEntityError, MetaEntityErrorCode};
use serum_meta_entity::instruction::MetaEntityInstruction;
use solana_sdk::account_info::AccountInfo;
use solana_sdk::entrypoint::ProgramResult;
use solana_sdk::pubkey::Pubkey;

mod initialize;
mod send_message;
mod update;

solana_sdk::entrypoint!(entry);
fn entry(program_id: &Pubkey, accounts: &[AccountInfo], instruction_data: &[u8]) -> ProgramResult {
    let mut data: &[u8] = instruction_data;
    let instruction: MetaEntityInstruction = MetaEntityInstruction::unpack_unchecked(&mut data)
        .map_err(|_| MetaEntityError::ErrorCode(MetaEntityErrorCode::WrongSerialization))?;

    let result = match instruction {
        MetaEntityInstruction::Initialize {
            entity,
            authority,
            name,
            about,
            image_url,
            chat,
        } => initialize::handler(
            program_id, accounts, entity, authority, name, about, image_url, chat,
        ),
        MetaEntityInstruction::Update {
            name,
            about,
            chat,
            image_url,
        } => update::handler(program_id, accounts, name, about, image_url, chat),
        MetaEntityInstruction::SendMessage { msg } => {
            send_message::handler(program_id, accounts, msg)
        }
    };

    result?;

    Ok(())
}
