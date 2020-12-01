use crate::access_control;
use serum_common::pack::Pack;
use serum_lockup::accounts::{vault, Vesting};
use serum_lockup::error::{LockupError, LockupErrorCode};
use solana_program::info;
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::program_pack::Pack as TokenPack;
use solana_sdk::pubkey::Pubkey;
use std::convert::Into;

pub fn handler(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: Vec<u8>,
) -> Result<(), LockupError> {
    info!("handler: whitelist_deposit");

    let acc_infos = &mut accounts.iter();

    let beneficiary_acc_info = next_account_info(acc_infos)?;
    let vesting_acc_info = next_account_info(acc_infos)?;
    let safe_acc_info = next_account_info(acc_infos)?;
    let wl_acc_info = next_account_info(acc_infos)?;
    let wl_prog_acc_info = next_account_info(acc_infos)?;

    // Below accounts are relayed.

    // Whitelist interface.
    let vault_acc_info = next_account_info(acc_infos)?;
    let vault_auth_acc_info = next_account_info(acc_infos)?;
    let tok_prog_acc_info = next_account_info(acc_infos)?;
    let wl_prog_vault_authority_acc_info = next_account_info(acc_infos)?;

    // Program specific.
    let remaining_relay_accs: Vec<&AccountInfo> = acc_infos.collect();

    access_control(AccessControlRequest {
        program_id,
        beneficiary_acc_info,
        vesting_acc_info,
        wl_acc_info,
        wl_prog_acc_info,
        wl_prog_vault_authority_acc_info,
        safe_acc_info,
        vault_acc_info,
        vault_auth_acc_info,
    })?;

    Vesting::unpack_unchecked_mut(
        &mut vesting_acc_info.try_borrow_mut_data()?,
        &mut |vesting: &mut Vesting| {
            state_transition(StateTransitionRequest {
                accounts,
                instruction_data: instruction_data.clone(),
                safe_acc: safe_acc_info.key,
                nonce: vesting.nonce,
                wl_prog_acc_info,
                wl_prog_vault_authority_acc_info,
                vault_acc_info,
                vault_auth_acc_info,
                tok_prog_acc_info,
                vesting,
                beneficiary_acc_info,
                remaining_relay_accs: remaining_relay_accs.clone(),
            })
            .map_err(Into::into)
        },
    )?;

    Ok(())
}

fn access_control(req: AccessControlRequest) -> Result<(), LockupError> {
    info!("access-control: whitelist_deposit");

    let AccessControlRequest {
        program_id,
        beneficiary_acc_info,
        vesting_acc_info,
        wl_acc_info,
        wl_prog_acc_info,
        wl_prog_vault_authority_acc_info,
        safe_acc_info,
        vault_acc_info,
        vault_auth_acc_info,
    } = req;

    // Beneficiary authorization.
    if !beneficiary_acc_info.is_signer {
        return Err(LockupErrorCode::Unauthorized)?;
    }

    // Account validation.
    let safe = access_control::safe(safe_acc_info, program_id)?;
    let whitelist =
        access_control::whitelist(wl_acc_info.clone(), safe_acc_info, &safe, program_id)?;
    let _vault = access_control::vault(
        vault_acc_info,
        vault_auth_acc_info,
        vesting_acc_info,
        beneficiary_acc_info,
        safe_acc_info,
        program_id,
    )?;
    let _vesting = access_control::vesting(
        program_id,
        safe_acc_info.key,
        vesting_acc_info,
        beneficiary_acc_info,
    )?;

    // WhitelistDeposit checks.
    let entry = whitelist
        .get_derived(wl_prog_vault_authority_acc_info.key)?
        .ok_or(LockupErrorCode::WhitelistNotFound)?;
    if entry.program_id() != *wl_prog_acc_info.key {
        return Err(LockupErrorCode::WhitelistInvalidProgramId)?;
    }

    info!("access-control: success");

    Ok(())
}

fn state_transition(req: StateTransitionRequest) -> Result<(), LockupError> {
    info!("state-transition: whitelist_deposit");

    let StateTransitionRequest {
        vesting,
        instruction_data,
        accounts,
        nonce,
        safe_acc,
        vault_acc_info,
        vault_auth_acc_info,
        wl_prog_acc_info,
        wl_prog_vault_authority_acc_info,
        remaining_relay_accs,
        tok_prog_acc_info,
        beneficiary_acc_info,
    } = req;

    // Check before balance.
    let before_amount = {
        let vault = spl_token::state::Account::unpack(&vault_acc_info.try_borrow_data()?)?;
        vault.amount
    };

    // Invoke relay, signing with the program-derived-address.
    {
        let mut meta_accounts = vec![
            AccountMeta::new(*vault_acc_info.key, false),
            AccountMeta::new_readonly(*vault_auth_acc_info.key, true),
            AccountMeta::new_readonly(*tok_prog_acc_info.key, false),
            AccountMeta::new_readonly(*wl_prog_vault_authority_acc_info.key, false),
        ];
        for a in remaining_relay_accs {
            if a.is_writable {
                meta_accounts.push(AccountMeta::new(*a.key, a.is_signer));
            } else {
                meta_accounts.push(AccountMeta::new_readonly(*a.key, a.is_signer));
            }
        }
        let mut data = serum_lockup::instruction::TAG.to_le_bytes().to_vec();
        data.extend(instruction_data);
        let relay_instruction = Instruction {
            program_id: *wl_prog_acc_info.key,
            accounts: meta_accounts,
            data,
        };
        let signer_seeds = vault::signer_seeds(safe_acc, beneficiary_acc_info.key, &nonce);
        solana_sdk::program::invoke_signed(&relay_instruction, &accounts[..], &[&signer_seeds])?;
    }

    // Update vesting account with the deposit.
    {
        let vault = spl_token::state::Account::unpack(&vault_acc_info.try_borrow_data()?)?;
        let deposit_amount = vault.amount - before_amount;

        // Safety checks.
        //
        // Balance must go up.
        if deposit_amount <= 0 {
            return Err(LockupErrorCode::InsufficientDepositAmount)?;
        }
        // Cannot deposit more than withdrawn.
        if deposit_amount > vesting.whitelist_owned {
            return Err(LockupErrorCode::DepositOverflow)?;
        }

        vesting.whitelist_owned -= deposit_amount;
    }

    Ok(())
}

struct AccessControlRequest<'a, 'b> {
    program_id: &'a Pubkey,
    wl_acc_info: &'a AccountInfo<'b>,
    wl_prog_acc_info: &'a AccountInfo<'b>,
    wl_prog_vault_authority_acc_info: &'a AccountInfo<'b>,
    beneficiary_acc_info: &'a AccountInfo<'b>,
    vesting_acc_info: &'a AccountInfo<'b>,
    safe_acc_info: &'a AccountInfo<'b>,
    vault_acc_info: &'a AccountInfo<'b>,
    vault_auth_acc_info: &'a AccountInfo<'b>,
}

struct StateTransitionRequest<'a, 'b, 'c> {
    instruction_data: Vec<u8>,
    vesting: &'c mut Vesting,
    accounts: &'a [AccountInfo<'b>],
    nonce: u8,
    safe_acc: &'a Pubkey,
    vault_acc_info: &'a AccountInfo<'b>,
    vault_auth_acc_info: &'a AccountInfo<'b>,
    wl_prog_acc_info: &'a AccountInfo<'b>,
    wl_prog_vault_authority_acc_info: &'a AccountInfo<'b>,
    remaining_relay_accs: Vec<&'a AccountInfo<'b>>,
    tok_prog_acc_info: &'a AccountInfo<'b>,
    beneficiary_acc_info: &'a AccountInfo<'b>,
}
