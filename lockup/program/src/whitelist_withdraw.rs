use crate::common::{access_control, whitelist_cpi};
use serum_common::pack::Pack;
use serum_lockup::accounts::Vesting;
use serum_lockup::error::{LockupError, LockupErrorCode};
use solana_program::msg;
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::program_pack::Pack as TokenPack;
use solana_sdk::pubkey::Pubkey;
use spl_token::state::Account as TokenAccount;

pub fn handler(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    amount: u64,
    ref instruction_data: Vec<u8>,
) -> Result<(), LockupError> {
    msg!("handler: whitelist_withdraw");

    let acc_infos = &mut accounts.iter();

    let beneficiary_acc_info = next_account_info(acc_infos)?;
    let safe_acc_info = next_account_info(acc_infos)?;
    let wl_acc_info = next_account_info(acc_infos)?;
    let wl_prog_acc_info = next_account_info(acc_infos)?;

    // Below accounts are relayed.

    // Whitelist interface.
    let vesting_acc_info = next_account_info(acc_infos)?;
    let vault_acc_info = next_account_info(acc_infos)?;
    let vault_auth_acc_info = next_account_info(acc_infos)?;
    let tok_prog_acc_info = next_account_info(acc_infos)?;
    let wl_prog_vault_acc_info = next_account_info(acc_infos)?;
    let wl_prog_vault_authority_acc_info = next_account_info(acc_infos)?;

    // Program specific.
    let remaining_relay_accs = acc_infos;

    let AccessControlResponse { vesting_nonce } = access_control(AccessControlRequest {
        program_id,
        beneficiary_acc_info,
        vesting_acc_info,
        wl_acc_info,
        wl_prog_acc_info,
        wl_prog_vault_acc_info,
        wl_prog_vault_authority_acc_info,
        safe_acc_info,
        vault_acc_info,
        vault_auth_acc_info,
        amount,
    })?;
    state_transition(StateTransitionRequest {
        accounts,
        amount,
        vesting_nonce,
        vesting_acc_info,
        instruction_data,
        safe_acc_info,
        wl_prog_acc_info,
        wl_prog_vault_acc_info,
        wl_prog_vault_authority_acc_info,
        vault_acc_info,
        vault_auth_acc_info,
        tok_prog_acc_info,
        beneficiary_acc_info,
        remaining_relay_accs,
    })?;

    Ok(())
}

fn access_control(req: AccessControlRequest) -> Result<AccessControlResponse, LockupError> {
    msg!("access-control: whitelist_withdraw");

    let AccessControlRequest {
        program_id,
        beneficiary_acc_info,
        vesting_acc_info,
        wl_acc_info,
        wl_prog_vault_acc_info,
        wl_prog_acc_info,
        wl_prog_vault_authority_acc_info,
        safe_acc_info,
        vault_acc_info,
        vault_auth_acc_info,
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
    let _vault = access_control::vault(
        vault_acc_info,
        vault_auth_acc_info,
        vesting_acc_info,
        beneficiary_acc_info,
        safe_acc_info,
        program_id,
    )?;
    let vesting = access_control::vesting(
        program_id,
        safe_acc_info,
        vesting_acc_info,
        beneficiary_acc_info,
    )?;

    // WhitelistWithdraw checks.
    //
    // Do we have sufficient balance?
    if amount > vesting.balance() {
        return Err(LockupErrorCode::InsufficientWhitelistBalance)?;
    }
    // Is the given program on the whitelist?
    let entry = whitelist
        .get_derived(wl_prog_vault_authority_acc_info.key)?
        .ok_or(LockupErrorCode::WhitelistNotFound)?;
    if entry.program_id() != *wl_prog_acc_info.key {
        return Err(LockupErrorCode::WhitelistInvalidProgramId)?;
    }
    // Is the vault owned by this whitelisted authority?
    let wl_vault = access_control::token(wl_prog_vault_acc_info)?;
    if &wl_vault.owner != wl_prog_vault_authority_acc_info.key {
        return Err(LockupErrorCode::InvalidTokenAccountOwner)?;
    }

    Ok(AccessControlResponse {
        vesting_nonce: vesting.nonce,
    })
}

fn state_transition(req: StateTransitionRequest) -> Result<(), LockupError> {
    msg!("state-transition: whitelist_withdraw");

    let StateTransitionRequest {
        vesting_acc_info,
        vesting_nonce,
        instruction_data,
        beneficiary_acc_info,
        accounts,
        amount,
        safe_acc_info,
        vault_acc_info,
        wl_prog_acc_info,
        wl_prog_vault_acc_info,
        wl_prog_vault_authority_acc_info,
        remaining_relay_accs,
        tok_prog_acc_info,
        vault_auth_acc_info,
    } = req;

    let before_amount = {
        let vault = TokenAccount::unpack(&vault_acc_info.try_borrow_data()?)?;
        vault.amount
    };

    // Invoke relay.
    {
        let mut meta_accounts = vec![
            AccountMeta::new_readonly(*vesting_acc_info.key, false),
            AccountMeta::new(*vault_acc_info.key, false),
            AccountMeta::new_readonly(*vault_auth_acc_info.key, true),
            AccountMeta::new_readonly(*tok_prog_acc_info.key, false),
            AccountMeta::new(*wl_prog_vault_acc_info.key, false),
            AccountMeta::new_readonly(*wl_prog_vault_authority_acc_info.key, false),
        ];
        meta_accounts.extend(remaining_relay_accs.map(|a| {
            if a.is_writable {
                AccountMeta::new(*a.key, a.is_signer)
            } else {
                AccountMeta::new_readonly(*a.key, a.is_signer)
            }
        }));
        let relay_instruction = Instruction {
            program_id: *wl_prog_acc_info.key,
            accounts: meta_accounts,
            data: instruction_data.to_vec(),
        };
        whitelist_cpi(
            relay_instruction,
            safe_acc_info.key,
            beneficiary_acc_info,
            vesting_nonce,
            accounts,
        )?;
    }

    let after_amount = {
        let vault = TokenAccount::unpack(&vault_acc_info.try_borrow_data()?)?;
        vault.amount
    };

    // Withdrawal safety checks.
    let amount_transferred = before_amount - after_amount;
    // Is the amount transferred valid?
    if amount_transferred > amount {
        return Err(LockupErrorCode::InsufficientAmount)?;
    }

    Vesting::unpack_unchecked_mut(
        &mut vesting_acc_info.try_borrow_mut_data()?,
        &mut |vesting: &mut Vesting| {
            vesting.whitelist_owned += amount_transferred;
            Ok(())
        },
    )?;

    Ok(())
}

struct AccessControlRequest<'a, 'b> {
    wl_prog_vault_acc_info: &'a AccountInfo<'b>,
    beneficiary_acc_info: &'a AccountInfo<'b>,
    vesting_acc_info: &'a AccountInfo<'b>,
    safe_acc_info: &'a AccountInfo<'b>,
    vault_acc_info: &'a AccountInfo<'b>,
    vault_auth_acc_info: &'a AccountInfo<'b>,
    wl_acc_info: &'a AccountInfo<'b>,
    wl_prog_acc_info: &'a AccountInfo<'b>,
    wl_prog_vault_authority_acc_info: &'a AccountInfo<'b>,
    program_id: &'a Pubkey,
    amount: u64,
}

struct AccessControlResponse {
    vesting_nonce: u8,
}

struct StateTransitionRequest<'a, 'b, 'c> {
    remaining_relay_accs: &'c mut dyn Iterator<Item = &'a AccountInfo<'b>>,
    accounts: &'a [AccountInfo<'b>],
    vault_acc_info: &'a AccountInfo<'b>,
    wl_prog_acc_info: &'a AccountInfo<'b>,
    wl_prog_vault_acc_info: &'a AccountInfo<'b>,
    wl_prog_vault_authority_acc_info: &'a AccountInfo<'b>,
    tok_prog_acc_info: &'a AccountInfo<'b>,
    vault_auth_acc_info: &'a AccountInfo<'b>,
    beneficiary_acc_info: &'a AccountInfo<'b>,
    safe_acc_info: &'a AccountInfo<'b>,
    vesting_acc_info: &'a AccountInfo<'b>,
    instruction_data: &'c [u8],
    vesting_nonce: u8,
    amount: u64,
}
