use crate::access_control;
use serum_common::pack::Pack;
use serum_lockup::accounts::{Safe, TokenVault, Vesting};
use serum_lockup::error::{LockupError, LockupErrorCode};
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::info;
use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::program_pack::Pack as TokenPack;
use solana_sdk::pubkey::Pubkey;
use std::convert::Into;

pub fn handler<'a>(
    program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'a>],
    amount: u64,
    instruction_data: Vec<u8>,
) -> Result<(), LockupError> {
    info!("handler: whitelist_withdraw");

    let acc_infos = &mut accounts.iter();

    let beneficiary_acc_info = next_account_info(acc_infos)?;
    let vesting_acc_info = next_account_info(acc_infos)?;
    let safe_acc_info = next_account_info(acc_infos)?;
    let safe_vault_auth_acc_info = next_account_info(acc_infos)?;
    let wl_prog_acc_info = next_account_info(acc_infos)?;
    let wl_acc_info = next_account_info(acc_infos)?;

    // Below accounts are relayed.

    let safe_vault_acc_info = next_account_info(acc_infos)?;
    let wl_prog_vault_acc_info = next_account_info(acc_infos)?;
    let wl_prog_vault_authority_acc_info = next_account_info(acc_infos)?;
    let tok_prog_acc_info = next_account_info(acc_infos)?;

    let remaining_relay_accs: Vec<&AccountInfo> = acc_infos.collect();

    access_control(AccessControlRequest {
        program_id,
        beneficiary_acc_info,
        vesting_acc_info,
        wl_acc_info,
        wl_prog_acc_info,
        wl_prog_vault_authority_acc_info,
        safe_acc_info,
        safe_vault_auth_acc_info,
        safe_vault_acc_info,
        amount,
    })?;

    Vesting::unpack_mut(
        &mut vesting_acc_info.try_borrow_mut_data()?,
        &mut |vesting: &mut Vesting| {
            let safe = Safe::unpack(&safe_acc_info.try_borrow_data()?)?;
            state_transition(StateTransitionRequest {
                accounts,
                amount,
                instruction_data: instruction_data.clone(),
                safe_acc: safe_acc_info.key,
                nonce: safe.nonce,
                wl_prog_acc_info,
                wl_prog_vault_acc_info,
                wl_prog_vault_authority_acc_info,
                safe_vault_acc_info,
                safe_vault_auth_acc_info,
                tok_prog_acc_info,
                vesting,
                remaining_relay_accs: remaining_relay_accs.clone(),
            })
            .map_err(Into::into)
        },
    )?;

    Ok(())
}

fn access_control(req: AccessControlRequest) -> Result<(), LockupError> {
    info!("access-control: whitelist_withdraw");

    let AccessControlRequest {
        program_id,
        beneficiary_acc_info,
        vesting_acc_info,
        wl_acc_info,
        wl_prog_acc_info,
        wl_prog_vault_authority_acc_info,
        safe_acc_info,
        safe_vault_auth_acc_info,
        safe_vault_acc_info,
        amount,
    } = req;

    // Beneficiary authorization.
    if !beneficiary_acc_info.is_signer {
        return Err(LockupErrorCode::Unauthorized)?;
    }

    // Account validation.
    let safe = access_control::safe(safe_acc_info, program_id)?;
    let whitelist =
        access_control::whitelist(wl_acc_info.clone(), safe_acc_info, &safe, program_id)?;
    let _ = access_control::vault(
        safe_vault_acc_info,
        safe_vault_auth_acc_info,
        safe_acc_info,
        program_id,
    )?;
    let vesting = access_control::vesting(
        program_id,
        safe_acc_info.key,
        vesting_acc_info,
        beneficiary_acc_info,
    )?;

    // WhitelistWithdraw checks.
    if !vesting.claimed {
        return Err(LockupErrorCode::NotYetClaimed)?;
    }
    if amount > vesting.available_for_whitelist() {
        return Err(LockupErrorCode::InsufficientWhitelistBalance)?;
    }
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
    info!("state-transition: whitelist_withdraw");

    let StateTransitionRequest {
        vesting,
        instruction_data,
        accounts,
        amount,
        nonce,
        safe_acc,
        safe_vault_acc_info,
        wl_prog_acc_info,
        wl_prog_vault_acc_info,
        wl_prog_vault_authority_acc_info,
        remaining_relay_accs,
        tok_prog_acc_info,
        safe_vault_auth_acc_info,
    } = req;

    let signer_seeds = TokenVault::signer_seeds(safe_acc, &nonce);

    // Approve delegate access for the amount.
    {
        info!("approving delegate");
        let approve_instr = spl_token::instruction::approve(
            &tok_prog_acc_info.key,
            &safe_vault_acc_info.key,
            &wl_prog_vault_authority_acc_info.key,
            &safe_vault_auth_acc_info.key,
            &[],
            amount,
        )?;
        solana_sdk::program::invoke_signed(&approve_instr, &accounts[..], &[&signer_seeds])?;
    }

    // Invoke relay.
    {
        info!("invoking relay");
        let mut meta_accounts = vec![
            AccountMeta::new_readonly(*safe_vault_auth_acc_info.key, true),
            AccountMeta::new(*safe_vault_acc_info.key, false),
            AccountMeta::new(*wl_prog_vault_acc_info.key, false),
            AccountMeta::new_readonly(*wl_prog_vault_authority_acc_info.key, false),
            AccountMeta::new_readonly(*tok_prog_acc_info.key, false),
        ];
        for a in remaining_relay_accs {
            if a.is_writable {
                meta_accounts.push(AccountMeta::new(*a.key, a.is_signer));
            } else {
                meta_accounts.push(AccountMeta::new_readonly(*a.key, a.is_signer));
            }
        }
        let relay_instruction = Instruction {
            program_id: *wl_prog_acc_info.key,
            accounts: meta_accounts,
            data: instruction_data,
        };

        solana_sdk::program::invoke_signed(&relay_instruction, &accounts[..], &[&signer_seeds])?;
    }

    // Revoke delegate access.
    {
        let revoke_instr = spl_token::instruction::revoke(
            &tok_prog_acc_info.key,
            &safe_vault_acc_info.key,
            &safe_vault_auth_acc_info.key,
            &[],
        )?;
        solana_sdk::program::invoke_signed(&revoke_instr, &accounts[..], &[&signer_seeds])?;
    }

    // Update vesting account.
    {
        let vault = spl_token::state::Account::unpack(&safe_vault_acc_info.try_borrow_data()?)?;
        let amount_transferred = amount - vault.delegated_amount;
        vesting.whitelist_owned += amount_transferred;
    }

    info!("state-transition: success");

    Ok(())
}

struct AccessControlRequest<'a> {
    program_id: &'a Pubkey,
    beneficiary_acc_info: &'a AccountInfo<'a>,
    vesting_acc_info: &'a AccountInfo<'a>,
    safe_acc_info: &'a AccountInfo<'a>,
    safe_vault_acc_info: &'a AccountInfo<'a>,
    safe_vault_auth_acc_info: &'a AccountInfo<'a>,
    wl_acc_info: &'a AccountInfo<'a>,
    wl_prog_acc_info: &'a AccountInfo<'a>,
    wl_prog_vault_authority_acc_info: &'a AccountInfo<'a>,
    amount: u64,
}

struct StateTransitionRequest<'a, 'b> {
    instruction_data: Vec<u8>,
    vesting: &'b mut Vesting,
    accounts: &'a [AccountInfo<'a>],
    amount: u64,
    nonce: u8,
    safe_acc: &'a Pubkey,
    safe_vault_acc_info: &'a AccountInfo<'a>,
    safe_vault_auth_acc_info: &'a AccountInfo<'a>,
    wl_prog_acc_info: &'a AccountInfo<'a>,
    wl_prog_vault_acc_info: &'a AccountInfo<'a>,
    wl_prog_vault_authority_acc_info: &'a AccountInfo<'a>,
    remaining_relay_accs: Vec<&'a AccountInfo<'a>>,
    tok_prog_acc_info: &'a AccountInfo<'a>,
}
