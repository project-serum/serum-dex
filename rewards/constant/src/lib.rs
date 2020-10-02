//! Program entrypoint.
//!
//! A constant rewards program. It will always return the same rewards everytime
//! it is invoked. Used for testing.
//!
//! The rewards program runs a single function, the calculation of node rewards.
//! There are no additionally defined instructions.
//!
//! This program must obey the interface assumed by the `Registry` program.
//!
//! Instruction data is none.
//!
//! Accounts:
//!
//! 0. `[writable]` ReturnValue account since Solana doesn't actually let you
//!                 return from cross program invocations.
//! 1. `[]`         Registry instance.
//! 2. `[]`         Stake account from which to get rewards.
//! 3. `[]`         Entity the stake account is associated with.
//!
//! Where the `ReturnValue` account has an 8 byte data array representing
//! a little endian u64.

#![cfg_attr(feature = "strict", deny(warnings))]

use error::RewardsError;
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::entrypoint::ProgramResult;
use solana_sdk::info;
use solana_sdk::pubkey::Pubkey;

mod error;

// The rewards calculated by this program.
const REWARDS_CONSTANT: u64 = 100;

solana_sdk::entrypoint!(process_instruction);
fn process_instruction<'a>(
    program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'a>],
    _instruction_data: &[u8],
) -> ProgramResult {
    info!("process-instruction");
    handler(program_id, accounts)?;
    info!("process-instruction success");
    Ok(())
}

pub fn handler<'a>(
    _program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'a>],
) -> Result<(), RewardsError> {
    info!("handler: rewards");

    let acc_infos = &mut accounts.iter();

    let return_value_acc_info = next_account_info(acc_infos)?;
    let _registry_acc_info = next_account_info(acc_infos)?;
    let _stake_acc_info = next_account_info(acc_infos)?;
    let _entity_acc_info = next_account_info(acc_infos)?;

    let return_value_result = REWARDS_CONSTANT;

    access_control(AccessControlRequest {})?;

    state_transition(StateTransitionRequest {
        return_value_acc_info,
        return_value_result,
    })
    .map_err(Into::into)
}

fn access_control(req: AccessControlRequest) -> Result<(), RewardsError> {
    info!("access-control: rewards");

    let AccessControlRequest { .. } = req;

    // todo

    info!("access-control: success");

    Ok(())
}

fn state_transition(req: StateTransitionRequest) -> Result<(), RewardsError> {
    info!("state-transition: rewards");

    let StateTransitionRequest {
        return_value_acc_info,
        return_value_result,
    } = req;

    // Write the data into the return value account, since Solana doesn't
    // support returning data across program invocations in any other way.
    let mut rv_data = return_value_acc_info.try_borrow_mut_data()?;
    rv_data.copy_from_slice(&return_value_result.to_le_bytes());

    info!("state-transition: success");

    Ok(())
}

struct AccessControlRequest {}

struct StateTransitionRequest<'a> {
    return_value_acc_info: &'a AccountInfo<'a>,
    return_value_result: u64,
}
