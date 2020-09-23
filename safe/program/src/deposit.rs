use serum_common::pack::Pack;
use serum_safe::accounts::{SafeAccount, SrmVault, VestingAccount};
use serum_safe::error::{SafeError, SafeErrorCode};
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::info;
use solana_sdk::program_error::ProgramError;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::sysvar::rent::Rent;
use solana_sdk::sysvar::Sysvar;

pub fn handler<'a>(
    program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'a>],
    vesting_acc_beneficiary: Pubkey,
    vesting_slots: Vec<u64>,
    vesting_amounts: Vec<u64>,
) -> Result<(), SafeError> {
    info!("handler: deposit_srm");

    let acc_infos = &mut accounts.iter();

    let vesting_acc_info = next_account_info(acc_infos)?;
    let depositor_from = next_account_info(acc_infos)?;
    let authority_depositor_from = next_account_info(acc_infos)?;
    let safe_srm_vault_to = next_account_info(acc_infos)?;
    let safe_acc_info = next_account_info(acc_infos)?;
    let spl_token_program_acc_info = next_account_info(acc_infos)?;
    let rent_acc_info = next_account_info(acc_infos)?;

    access_control(AccessControlRequest {
        vesting_slots_len: vesting_slots.len(),
        program_id,
        vesting_acc_info,
        safe_acc_info,
        depositor_from,
        safe_srm_vault_to,
        rent_acc_info,
    })?;

    VestingAccount::unpack_mut(
        &mut vesting_acc_info.try_borrow_mut_data()?,
        &mut |vesting_acc: &mut VestingAccount| {
            state_transition(StateTransitionRequest {
                vesting_slots: vesting_slots.clone(),
                vesting_amounts: vesting_amounts.clone(),
                vesting_acc,
                vesting_acc_beneficiary,
                safe_acc_info,
                depositor_from,
                safe_srm_vault_to,
                authority_depositor_from,
                spl_token_program_acc_info,
            })?;
            Ok(())
        },
    )?;

    Ok(())
}

fn access_control<'a>(req: AccessControlRequest<'a>) -> Result<(), ProgramError> {
    info!("access-control: deposit");

    let AccessControlRequest {
        program_id,
        vesting_acc_info,
        safe_acc_info,
        depositor_from,
        safe_srm_vault_to,
        rent_acc_info,
        vesting_slots_len,
    } = req;

    let vesting_data = vesting_acc_info.try_borrow_data()?;

    // Check the dynamic data size is correct before unpacking.
    if vesting_data.len() != VestingAccount::data_size(vesting_slots_len)? as usize {
        return Err(SafeError::ErrorCode(SafeErrorCode::VestingAccountDataInvalid).into());
    }
    // Unsafe umpack.
    let vesting_acc = VestingAccount::unpack(&vesting_data)?;
    if vesting_acc.initialized {
        return Err(SafeError::ErrorCode(SafeErrorCode::AlreadyInitialized).into());
    }
    let rent = Rent::from_account_info(rent_acc_info)?;
    if !rent.is_exempt(vesting_acc_info.lamports(), vesting_data.len()) {
        return Err(SafeError::ErrorCode(SafeErrorCode::NotRentExempt).into());
    }
    if vesting_acc_info.owner != program_id {
        return Err(SafeError::ErrorCode(SafeErrorCode::NotOwnedByProgram).into());
    }
    // Look at the deposit's SPL account data and check for the mint.
    let safe_acc = SafeAccount::unpack(&safe_acc_info.try_borrow_data()?)
        .map_err(|_| SafeError::ErrorCode(SafeErrorCode::SafeAccountDataInvalid))?;
    if safe_acc.mint != Pubkey::new(&depositor_from.try_borrow_data()?[..32]) {
        return Err(SafeError::ErrorCode(SafeErrorCode::WrongCoinMint).into());
    }
    // Look into the safe vault's SPL account data and check for the owner (it should
    // be this program).
    let nonce = &[safe_acc.nonce];
    let seeds = SrmVault::signer_seeds(safe_acc_info.key, nonce);
    let expected_authority = Pubkey::create_program_address(&seeds, program_id)?;
    if Pubkey::new(&safe_srm_vault_to.try_borrow_data()?[32..64]) != expected_authority {
        return Err(SafeError::ErrorCode(SafeErrorCode::WrongVault).into());
    }

    info!("access-control: success");

    Ok(())
}

fn state_transition<'a, 'b>(req: StateTransitionRequest<'a, 'b>) -> Result<(), SafeError> {
    info!("state-transition: deposit");

    let StateTransitionRequest {
        vesting_acc,
        vesting_acc_beneficiary,
        safe_acc_info,
        vesting_slots,
        vesting_amounts,
        depositor_from,
        safe_srm_vault_to,
        authority_depositor_from,
        spl_token_program_acc_info,
    } = req;

    // Update account.
    vesting_acc.safe = safe_acc_info.key.clone();
    vesting_acc.beneficiary = vesting_acc_beneficiary;
    vesting_acc.initialized = true;
    vesting_acc.slots = vesting_slots.clone();
    vesting_acc.amounts = vesting_amounts.clone();

    let total_vest_amount = vesting_amounts.iter().sum();

    // Now transfer SPL funds from the depositor, to the
    // program-controlled-address.
    {
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
        assert_eq!(*spl_token_program_acc_info.key, spl_token::ID);
        solana_sdk::program::invoke_signed(
            &deposit_instruction,
            &[
                depositor_from.clone(),
                authority_depositor_from.clone(),
                safe_srm_vault_to.clone(),
                spl_token_program_acc_info.clone(),
            ],
            &[],
        )?;
    }

    info!("state-transition: complete");

    Ok(())
}

struct AccessControlRequest<'a> {
    program_id: &'a Pubkey,
    vesting_acc_info: &'a AccountInfo<'a>,
    safe_acc_info: &'a AccountInfo<'a>,
    depositor_from: &'a AccountInfo<'a>,
    safe_srm_vault_to: &'a AccountInfo<'a>,
    rent_acc_info: &'a AccountInfo<'a>,
    vesting_slots_len: usize,
}

struct StateTransitionRequest<'a, 'b> {
    vesting_acc: &'b mut VestingAccount,
    vesting_acc_beneficiary: Pubkey,
    safe_acc_info: &'a AccountInfo<'a>,
    vesting_slots: Vec<u64>,
    vesting_amounts: Vec<u64>,
    depositor_from: &'a AccountInfo<'a>,
    safe_srm_vault_to: &'a AccountInfo<'a>,
    authority_depositor_from: &'a AccountInfo<'a>,
    spl_token_program_acc_info: &'a AccountInfo<'a>,
}
