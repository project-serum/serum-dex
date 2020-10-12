//! A dummy staking program for testing. Implements the lockup program's
//! "whitelist" program interface, allowing the lockup program to relay
//! instructions to `Stake` and `Unstake`.

#![cfg_attr(feature = "strict", deny(warnings))]

use serum_common::pack::*;
use serum_lockup_test_stake::accounts;
use serum_lockup_test_stake::instruction::StakeInstruction;
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::entrypoint::ProgramResult;
#[cfg(feature = "program")]
use solana_sdk::info;
use solana_sdk::pubkey::Pubkey;

#[cfg(feature = "program")]
solana_sdk::entrypoint!(process_instruction);
#[cfg(feature = "program")]
fn process_instruction(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    info!("process-instruction");

    let instruction: StakeInstruction = StakeInstruction::unpack(instruction_data).unwrap();

    let result = match instruction {
        StakeInstruction::Initialize { nonce } => handlers::initialize(accounts, nonce),
        StakeInstruction::Stake { amount } => handlers::stake(accounts, amount),
        StakeInstruction::Unstake { amount } => handlers::unstake(accounts, amount),
    };

    result?;

    info!("process-instruction success");

    Ok(())
}

#[cfg(feature = "program")]
mod handlers {
    use super::*;
    pub fn initialize(accounts: &[AccountInfo], nonce: u8) -> ProgramResult {
        info!("handler: initialize");

        let acc_infos = &mut accounts.iter();
        let wl_acc_info = next_account_info(acc_infos)?;

        accounts::Instance::unpack_mut(
            &mut wl_acc_info.try_borrow_mut_data()?,
            &mut |wl: &mut accounts::Instance| {
                wl.nonce = nonce;
                Ok(())
            },
        )
    }

    pub fn stake(accounts: &[AccountInfo], amount: u64) -> ProgramResult {
        info!("handler: stake");
        let acc_infos = &mut accounts.iter();

        // Registry relay.
        let delegate_owner_acc_info = next_account_info(acc_infos)?;
        let token_acc_info = next_account_info(acc_infos)?;
        let vault_acc_info = next_account_info(acc_infos)?;
        let vault_authority_acc_info = next_account_info(acc_infos)?;
        let token_program_acc_info = next_account_info(acc_infos)?;

        // Program specific.
        let wl_acc_info = next_account_info(acc_infos)?;

        assert!(delegate_owner_acc_info.is_signer);

        let wl = accounts::Instance::unpack(&wl_acc_info.try_borrow_data()?)?;
        let nonce = wl.nonce;
        let signer_seeds = accounts::signer_seeds(wl_acc_info.key, &nonce);

        // Delegate transfer to oneself.
        let transfer_instruction = spl_token::instruction::transfer(
            &spl_token::ID,
            token_acc_info.key,
            vault_acc_info.key,
            &vault_authority_acc_info.key,
            &[],
            amount,
        )?;
        solana_sdk::program::invoke_signed(
            &transfer_instruction,
            &[
                vault_acc_info.clone(),
                token_acc_info.clone(),
                vault_authority_acc_info.clone(),
                token_program_acc_info.clone(),
            ],
            &[&signer_seeds],
        )
    }

    pub fn unstake(accounts: &[AccountInfo], amount: u64) -> ProgramResult {
        info!("handler: unstake");
        let acc_infos = &mut accounts.iter();

        let delegate_owner_acc_info = next_account_info(acc_infos)?;
        let token_acc_info = next_account_info(acc_infos)?;
        let vault_acc_info = next_account_info(acc_infos)?;
        let vault_authority_acc_info = next_account_info(acc_infos)?;
        let token_program_acc_info = next_account_info(acc_infos)?;
        let wl_acc_info = next_account_info(acc_infos)?;

        assert!(delegate_owner_acc_info.is_signer);

        let wl = accounts::Instance::unpack(&wl_acc_info.try_borrow_data()?)?;
        let nonce = wl.nonce;
        let signer_seeds = accounts::signer_seeds(wl_acc_info.key, &nonce);

        let transfer_instruction = spl_token::instruction::transfer(
            &spl_token::ID,
            vault_acc_info.key,
            token_acc_info.key,
            &vault_authority_acc_info.key,
            &[],
            amount,
        )?;
        solana_sdk::program::invoke_signed(
            &transfer_instruction,
            &[
                vault_acc_info.clone(),
                token_acc_info.clone(),
                vault_authority_acc_info.clone(),
                token_program_acc_info.clone(),
            ],
            &[&signer_seeds],
        )
    }
}
