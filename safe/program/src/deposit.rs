use serum_common::pack::Pack;
use serum_safe::accounts::{Safe, TokenVault, Vesting};
use serum_safe::error::{SafeError, SafeErrorCode};
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::info;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::sysvar::rent::Rent;
use solana_sdk::sysvar::Sysvar;
use spl_token::pack::Pack as TokenPack;
use std::convert::Into;

pub fn handler<'a>(
    program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'a>],
    vesting_acc_beneficiary: Pubkey,
    vesting_slots: Vec<u64>,
    vesting_amounts: Vec<u64>,
) -> Result<(), SafeError> {
    info!("handler: deposit");

    let acc_infos = &mut accounts.iter();

    let vesting_acc_info = next_account_info(acc_infos)?;
    let depositor_acc_info = next_account_info(acc_infos)?;
    let depositor_authority_acc_info = next_account_info(acc_infos)?;
    let safe_vault_acc_info = next_account_info(acc_infos)?;
    let safe_acc_info = next_account_info(acc_infos)?;
    let token_program_acc_info = next_account_info(acc_infos)?;
    let rent_acc_info = next_account_info(acc_infos)?;

    access_control(AccessControlRequest {
        vesting_slots: &vesting_slots,
        vesting_amounts: &vesting_amounts,
        program_id,
        vesting_acc_info,
        safe_acc_info,
        depositor_acc_info,
        depositor_authority_acc_info,
        safe_vault_acc_info,
        token_program_acc_info,
        rent_acc_info,
    })?;

    // Same deal with unpack_unchecked. See the comment in `access_control`
    // for safety considerations.
    Vesting::unpack_unchecked_mut(
        &mut vesting_acc_info.try_borrow_mut_data()?,
        &mut |vesting_acc: &mut Vesting| {
            state_transition(StateTransitionRequest {
                vesting_slots: vesting_slots.clone(),
                vesting_amounts: vesting_amounts.clone(),
                vesting_acc,
                vesting_acc_beneficiary,
                safe_acc_info,
                depositor_acc_info,
                safe_vault_acc_info,
                depositor_authority_acc_info,
                token_program_acc_info,
            })
            .map_err(Into::into)
        },
    )?;

    Ok(())
}

fn access_control<'a, 'b>(req: AccessControlRequest<'a, 'b>) -> Result<(), SafeError> {
    info!("access-control: deposit");

    let AccessControlRequest {
        program_id,
        vesting_acc_info,
        safe_acc_info,
        depositor_acc_info,
        safe_vault_acc_info,
        depositor_authority_acc_info,
        token_program_acc_info,
        rent_acc_info,
        vesting_slots,
        vesting_amounts,
    } = req;

    // Depositor authorization.
    {
        if !depositor_authority_acc_info.is_signer {
            return Err(SafeErrorCode::Unauthorized)?;
        }
    }

    // Safe.
    let safe = Safe::unpack(&safe_acc_info.try_borrow_data()?)?;
    {
        if !safe.initialized {
            return Err(SafeErrorCode::NotInitialized)?;
        }
    }

    // Vault.
    {
        let safe_vault =
            spl_token::state::Account::unpack(&safe_vault_acc_info.try_borrow_data()?)?;

        if *safe_vault_acc_info.owner != spl_token::ID {
            return Err(SafeErrorCode::InvalidVault)?;
        }
        if safe_vault.state != spl_token::state::AccountState::Initialized {
            return Err(SafeErrorCode::NotInitialized)?;
        }
        let safe_vault_authority = Pubkey::create_program_address(
            &TokenVault::signer_seeds(safe_acc_info.key, &safe.nonce),
            program_id,
        )
        .map_err(|_| SafeErrorCode::InvalidVault)?;
        if safe_vault.owner != safe_vault_authority {
            return Err(SafeErrorCode::InvalidVault)?;
        }
    }

    // Vesting.
    {
        let vesting_data = vesting_acc_info.try_borrow_data()?;

        // Check the account's data-dependent size is correct before unpacking.
        if vesting_data.len() != Vesting::size_dyn(vesting_slots.len())? as usize {
            return Err(SafeErrorCode::VestingAccountDataInvalid)?;
        }
        // Perform an unpack_unchecked--that is, unsafe--deserialization.
        //
        // We might lose information when deserializing from all zeroes, because
        // Vesting has variable length Vecs (i.e., if you deserializ vec![0; 100]),
        // it can deserialize to vec![0; 0], depending on the serializer. This
        // is the case for bincode serialization. In other words, we might *not*
        // use the entire data array upon deserializing here.
        //
        // As a result, we follow this with a check on the slots and amounts to
        // guarantee that all subsequent instructions deal with non-zero vecs
        // (thus making our serialization size deterministic). And so all further
        // instructions should use the safe `unpack` variant method.
        //
        // This latter check is nice to have anyway, to prevent useless deposits.
        //
        // Switch serializers if this is a problem.
        let vesting = Vesting::unpack_unchecked(&vesting_data)?;
        if vesting.initialized {
            return Err(SafeErrorCode::AlreadyInitialized)?;
        }
        if !vesting_slots
            .iter()
            .filter(|slot| **slot == 0)
            .collect::<Vec<&u64>>()
            .is_empty()
        {
            return Err(SafeErrorCode::InvalidVestingSlots)?;
        }
        if !vesting_amounts
            .iter()
            .filter(|slot| **slot == 0)
            .collect::<Vec<&u64>>()
            .is_empty()
        {
            return Err(SafeErrorCode::InvalidVestingAmounts)?;
        }
        if vesting_acc_info.owner != program_id {
            return Err(SafeErrorCode::NotOwnedByProgram)?;
        }
        let rent = Rent::from_account_info(rent_acc_info)?;
        if !rent.is_exempt(vesting_acc_info.lamports(), vesting_data.len()) {
            return Err(SafeErrorCode::NotRentExempt)?;
        }
    }

    // Token program.
    {
        if *token_program_acc_info.key != spl_token::ID {
            return Err(SafeErrorCode::InvalidTokenProgram)?;
        }
    }

    // Rent sysvar.
    {
        if *rent_acc_info.key != solana_sdk::sysvar::rent::id() {
            return Err(SafeErrorCode::InvalidRentSysvar)?;
        }
    }

    // Depositor.
    {
        let depositor = spl_token::state::Account::unpack(&depositor_acc_info.try_borrow_data()?)?;
        if safe.mint != depositor.mint {
            return Err(SafeErrorCode::WrongCoinMint)?;
        }
        // Let the spl token program handle the rest of the depositor.
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
        depositor_acc_info,
        safe_vault_acc_info,
        depositor_authority_acc_info,
        token_program_acc_info,
    } = req;

    // Initialize account.
    {
        vesting_acc.safe = safe_acc_info.key.clone();
        vesting_acc.beneficiary = vesting_acc_beneficiary;
        vesting_acc.initialized = true;
        vesting_acc.slots = vesting_slots.clone();
        vesting_acc.amounts = vesting_amounts.clone();
    }

    // Now transfer SPL funds from the depositor, to the
    // program-controlled vault.
    {
        info!("invoke SPL token transfer");

        let total_deposit = vesting_amounts.iter().sum();

        let deposit_instruction = spl_token::instruction::transfer(
            &spl_token::ID,
            depositor_acc_info.key,
            safe_vault_acc_info.key,
            depositor_authority_acc_info.key,
            &[],
            total_deposit,
        )?;
        solana_sdk::program::invoke_signed(
            &deposit_instruction,
            &[
                depositor_acc_info.clone(),
                depositor_authority_acc_info.clone(),
                safe_vault_acc_info.clone(),
                token_program_acc_info.clone(),
            ],
            &[],
        )?;
    }

    info!("state-transition: complete");

    Ok(())
}

struct AccessControlRequest<'a, 'b> {
    program_id: &'a Pubkey,
    vesting_acc_info: &'a AccountInfo<'a>,
    safe_acc_info: &'a AccountInfo<'a>,
    depositor_acc_info: &'a AccountInfo<'a>,
    depositor_authority_acc_info: &'a AccountInfo<'a>,
    safe_vault_acc_info: &'a AccountInfo<'a>,
    token_program_acc_info: &'a AccountInfo<'a>,
    rent_acc_info: &'a AccountInfo<'a>,
    vesting_slots: &'b [u64],
    vesting_amounts: &'b [u64],
}

struct StateTransitionRequest<'a, 'b> {
    vesting_acc: &'b mut Vesting,
    vesting_acc_beneficiary: Pubkey,
    safe_acc_info: &'a AccountInfo<'a>,
    vesting_slots: Vec<u64>,
    vesting_amounts: Vec<u64>,
    depositor_acc_info: &'a AccountInfo<'a>,
    safe_vault_acc_info: &'a AccountInfo<'a>,
    depositor_authority_acc_info: &'a AccountInfo<'a>,
    token_program_acc_info: &'a AccountInfo<'a>,
}
