use arrayref::array_mut_ref;
use serum_safe::accounts::{SafeAccount, SrmVault, VestingAccount};
use serum_safe::error::{SafeError, SafeErrorCode};
use serum_safe::pack::DynPack;
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::info;
use solana_sdk::program_error::ProgramError;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::sysvar::rent::Rent;
use solana_sdk::sysvar::Sysvar;
use spl_token::pack::Pack;

pub fn handler<'a>(
    program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'a>],
    vesting_account_beneficiary: Pubkey,
    vesting_slots: Vec<u64>,
    vesting_amounts: Vec<u64>,
) -> Result<(), SafeError> {
    info!("handler: deposit_srm");

    let account_info_iter = &mut accounts.iter();

    let vesting_account_info = next_account_info(account_info_iter)?;
    let depositor_from = next_account_info(account_info_iter)?;
    let authority_depositor_from = next_account_info(account_info_iter)?;
    let safe_srm_vault_to = next_account_info(account_info_iter)?;
    let safe_account_info = next_account_info(account_info_iter)?;
    let spl_token_program_account_info = next_account_info(account_info_iter)?;
    let rent = Rent::from_account_info(next_account_info(account_info_iter)?)?;

    let mut vesting_account_data = vesting_account_info.data.borrow_mut();
    let vesting_account_data_len = vesting_account_data.len();

    if vesting_account_data[VestingAccount::initialized_index()] == 1 {
        return Err(SafeError::ErrorCode(SafeErrorCode::AlreadyInitialized).into());
    }

    // Check the dynamic data size is correct.
    let expected_size = VestingAccount::data_size(vesting_slots.len());
    if vesting_account_data.len() != expected_size {
        return Err(SafeError::ErrorCode(SafeErrorCode::VestingAccountDataInvalid).into());
    }

    // Inject the size into the first 8 bytes, since the dynamic unpacker will
    // check that.
    {
        let len_dst = array_mut_ref![vesting_account_data, 0, 8];
        len_dst.copy_from_slice(&expected_size.to_le_bytes())
    }

    let safe_account = SafeAccount::unpack(&safe_account_info.try_borrow_data()?)
        .map_err(|_| SafeError::ErrorCode(SafeErrorCode::SafeAccountDataInvalid))?;

    VestingAccount::unpack_unchecked_mut(
        &mut vesting_account_data,
        &mut |vesting_account: &mut VestingAccount| {
            access_control(AccessControlRequest {
                program_id,
                vesting_account,
                vesting_account_info,
                vesting_account_data_len,
                safe_account: &safe_account,
                safe_account_info,
                depositor_from,
                safe_srm_vault_to,
                rent,
            })?;

            state_transition(StateTransitionRequest {
                vesting_account,
                vesting_account_beneficiary,
                safe_account_info,
                vesting_slots: vesting_slots.clone(),
                vesting_amounts: vesting_amounts.clone(),
                depositor_from,
                safe_srm_vault_to,
                authority_depositor_from,
                spl_token_program_account_info,
            })?;

            Ok(())
        },
    )?;

    Ok(())
}

fn access_control<'a, 'b, 'c>(req: AccessControlRequest<'a, 'b, 'c>) -> Result<(), ProgramError> {
    info!("access-control: deposit");

    let AccessControlRequest {
        program_id,
        vesting_account,
        vesting_account_info,
        vesting_account_data_len,
        safe_account,
        safe_account_info,
        depositor_from,
        safe_srm_vault_to,
        rent,
    } = req;

    if vesting_account.initialized {
        return Err(SafeError::ErrorCode(SafeErrorCode::AlreadyInitialized).into());
    }
    if !rent.is_exempt(vesting_account_info.lamports(), vesting_account_data_len) {
        return Err(SafeError::ErrorCode(SafeErrorCode::NotRentExempt).into());
    }
    if vesting_account_info.owner != program_id {
        return Err(SafeError::ErrorCode(SafeErrorCode::NotOwnedByProgram).into());
    }
    // Look at the deposit's SPL account data and check for the mint.
    if safe_account.mint != Pubkey::new(&depositor_from.try_borrow_data()?[..32]) {
        return Err(SafeError::ErrorCode(SafeErrorCode::WrongCoinMint).into());
    }
    // Look into the safe vault's SPL account data and check for the owner (it should
    // be this program).
    let nonce = &[safe_account.nonce];
    let seeds = SrmVault::signer_seeds(safe_account_info.key, nonce);
    let expected_authority = Pubkey::create_program_address(&seeds, program_id)?;
    if Pubkey::new(&safe_srm_vault_to.try_borrow_data()?[32..64]) != expected_authority {
        return Err(SafeError::ErrorCode(SafeErrorCode::WrongVault).into());
    }

    info!("access-control: success");

    Ok(())
}

struct AccessControlRequest<'a, 'b, 'c> {
    program_id: &'a Pubkey,
    vesting_account: &'b VestingAccount,
    vesting_account_info: &'a AccountInfo<'a>,
    vesting_account_data_len: usize,
    safe_account: &'c SafeAccount,
    safe_account_info: &'a AccountInfo<'a>,
    depositor_from: &'a AccountInfo<'a>,
    safe_srm_vault_to: &'a AccountInfo<'a>,
    rent: Rent,
}

fn state_transition<'a, 'b>(req: StateTransitionRequest<'a, 'b>) -> Result<(), SafeError> {
    info!("state-transition: deposit");

    let StateTransitionRequest {
        vesting_account,
        vesting_account_beneficiary,
        safe_account_info,
        vesting_slots,
        vesting_amounts,
        depositor_from,
        safe_srm_vault_to,
        authority_depositor_from,
        spl_token_program_account_info,
    } = req;

    // Update account.
    vesting_account.safe = safe_account_info.key.clone();
    vesting_account.beneficiary = vesting_account_beneficiary;
    vesting_account.initialized = true;
    vesting_account.slots = vesting_slots.clone();
    vesting_account.amounts = vesting_amounts.clone();

    let total_vest_amount = vesting_amounts.iter().sum();

    // Now transfer SPL funds from the depositor, to the
    // program-controlled-address.
    info!("invoke SPL token transfer");

    let deposit_instruction = spl_token::instruction::transfer(
        &spl_token::ID,
        depositor_from.key,
        safe_srm_vault_to.key,
        authority_depositor_from.key,
        &[],
        total_vest_amount,
    )
    .unwrap();
    assert_eq!(*spl_token_program_account_info.key, spl_token::ID);
    solana_sdk::program::invoke_signed(
        &deposit_instruction,
        &[
            depositor_from.clone(),
            authority_depositor_from.clone(),
            safe_srm_vault_to.clone(),
            spl_token_program_account_info.clone(),
        ],
        &[],
    )?;

    info!("SPL token transfer complete");
    info!("state-transition: complete");

    Ok(())
}

struct StateTransitionRequest<'a, 'b> {
    vesting_account: &'b mut VestingAccount,
    vesting_account_beneficiary: Pubkey,
    safe_account_info: &'a AccountInfo<'a>,
    vesting_slots: Vec<u64>,
    vesting_amounts: Vec<u64>,
    depositor_from: &'a AccountInfo<'a>,
    safe_srm_vault_to: &'a AccountInfo<'a>,
    authority_depositor_from: &'a AccountInfo<'a>,
    spl_token_program_account_info: &'a AccountInfo<'a>,
}
