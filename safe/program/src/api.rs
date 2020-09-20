//! api.rs defines all instruction handlers for the program.

use arrayref::array_mut_ref;
use safe_transmute::to_bytes::transmute_to_bytes;
use serum_safe::accounts::{LsrmReceipt, SafeAccount, SrmVault, VestingAccount, Whitelist};
use serum_safe::error::{SafeError, SafeErrorCode};
use serum_safe::pack::DynPack;
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::info;
use solana_sdk::program_error::ProgramError;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::sysvar::rent::Rent;
use solana_sdk::sysvar::Sysvar;
use spl_token::pack::{IsInitialized, Pack};

pub fn initialize(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    mint: Pubkey,
    authority: Pubkey,
) -> Result<(), SafeError> {
    info!("HANDLER: initialize");
    let account_info_iter = &mut accounts.iter();
    let safe_account_info = next_account_info(account_info_iter)?;
    let safe_account_data_len = safe_account_info.data_len();
    let rent = &Rent::from_account_info(next_account_info(account_info_iter)?)?;

    let mut safe_account_data = safe_account_info.data.borrow_mut();
    SafeAccount::unpack_unchecked_mut(
        &mut safe_account_data,
        &mut |safe_account: &mut SafeAccount| {
            if safe_account.is_initialized {
                info!("ERROR: safe account already initialized");
                return Err(SafeError::ErrorCode(SafeErrorCode::AlreadyInitialized).into());
            }
            if !rent.is_exempt(safe_account_info.lamports(), safe_account_data_len) {
                info!("ERROR: safe account is not rent exempt");
                return Err(SafeError::ErrorCode(SafeErrorCode::NotRentExempt).into());
            }
            if safe_account_info.owner != program_id {
                info!("ERROR: safe account owner is not the program id");
                return Err(SafeError::ErrorCode(SafeErrorCode::NotOwnedByProgram).into());
            }

            safe_account.mint = mint;
            safe_account.is_initialized = true;
            safe_account.supply = 0;
            safe_account.authority = authority;
            safe_account.whitelist = Whitelist::zeroed();

            info!("safe initialization complete");

            Ok(())
        },
    )
    .map_err(|e| SafeError::ProgramError(e))
}

pub fn deposit_srm(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    vesting_account_beneficiary: Pubkey,
    vesting_slots: Vec<u64>,
    vesting_amounts: Vec<u64>,
) -> Result<(), SafeError> {
    info!("HANDLER: deposit_srm");

    let account_info_iter = &mut accounts.iter();

    let vesting_account_info = next_account_info(account_info_iter)?;
    let depositor_from = next_account_info(account_info_iter)?;
    let authority_depositor_from = next_account_info(account_info_iter)?;
    let safe_srm_vault_to = next_account_info(account_info_iter)?;
    let safe_account_info = next_account_info(account_info_iter)?;
    let spl_token_program_account_info = next_account_info(account_info_iter)?;
    let rent = &Rent::from_account_info(next_account_info(account_info_iter)?)?;

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
        let len: u64 = expected_size as u64;
        len_dst.copy_from_slice(&expected_size.to_le_bytes())
    }

    let safe_account = SafeAccount::unpack(&safe_account_info.try_borrow_data()?)
        .map_err(|_| SafeError::ErrorCode(SafeErrorCode::SafeAccountDataInvalid))?;

    VestingAccount::unpack_unchecked_mut(
        &mut vesting_account_data,
        &mut |vesting_account: &mut VestingAccount| {
            deposit_srm_access_control(
                program_id,
                vesting_account,
                vesting_account_info,
                vesting_account_data_len,
                &safe_account,
                safe_account_info,
                depositor_from,
                safe_srm_vault_to,
                rent,
            )?;
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
            let transfer_result = {
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
                )
            };
            info!("SPL token transfer complete");
            transfer_result?;
            Ok(())
        },
    )?;

    info!("deposit_srm complete");
    Ok(())
}

fn deposit_srm_access_control(
    program_id: &Pubkey,
    vesting_account: &VestingAccount,
    vesting_account_info: &AccountInfo,
    vesting_account_data_len: usize,
    safe_account: &SafeAccount,
    safe_account_info: &AccountInfo,
    depositor_from: &AccountInfo,
    safe_srm_vault_to: &AccountInfo,
    rent: &Rent,
) -> Result<(), ProgramError> {
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
    //
    // TODO: enable this.
    /*if Pubkey::new(&safe_srm_vault_to.try_borrow_data()?[32..64])
        != SrmVault::program_derived_address(program_id, safe_account_info.key)
    {
        return Err(SafeError::ErrorCode(SafeErrorCode::WrongVaultAddress).into());
    }*/
    Ok(())
}

pub fn mint_locked_srm(program_id: &Pubkey, accounts: &[AccountInfo]) -> Result<(), SafeError> {
    info!("HANDLER: mint_locked_srm");

    let accounts_len = accounts.len();
    if (accounts_len - 5) % 2 != 0 {
        return Err(SafeError::ErrorCode(SafeErrorCode::WrongNumberOfAccounts));
    }
    let lsrm_nft_count = ((accounts_len - 5) / 2) as u64;

    let account_info_iter = &mut accounts.iter();

    let vesting_account_beneficiary_info = next_account_info(account_info_iter)?;
    let vesting_account_info = next_account_info(account_info_iter)?;
    let safe_account_info = next_account_info(account_info_iter)?;
    let spl_token_program_account_info = next_account_info(account_info_iter)?;

    let rent_account_info = next_account_info(account_info_iter)?;
    let rent = &Rent::from_account_info(rent_account_info)?;

    let mut lsrm_nfts = vec![];

    let mut idx = 0;
    for _ in 0..lsrm_nft_count {
        let lsrm_spl_mint_info = next_account_info(account_info_iter)?;
        let lsrm_receipt_info = next_account_info(account_info_iter)?;
        lsrm_nfts.push((lsrm_spl_mint_info.clone(), lsrm_receipt_info));
    }

    VestingAccount::unpack_mut(
        &mut vesting_account_info.try_borrow_mut_data()?,
        &mut |vesting_account: &mut VestingAccount| {
            mint_locked_srm_access_control(
                vesting_account,
                vesting_account_beneficiary_info,
                &lsrm_nfts,
                lsrm_nft_count,
                spl_token_program_account_info,
                rent,
                rent_account_info,
            )?;

            for (mint, receipt) in &lsrm_nfts {
                LsrmReceipt::unpack_unchecked_mut(
                    &mut receipt.try_borrow_mut_data()?,
                    &mut |receipt: &mut LsrmReceipt| {
                        // Initialize the receipt.
                        {
                            receipt.initialized = true;
                            receipt.mint = *mint.key;
                            receipt.vesting_account = *vesting_account_info.key;
                            receipt.burned = false;
                        }
                        // Initialize the NFT mint.
                        {
                            let init_mint_instr = spl_token::instruction::initialize_mint(
                                &spl_token::ID,
                                &mint.key,
                                &program_id.clone(),
                                None,
                                0,
                            )
                            .unwrap();
                            solana_sdk::program::invoke(&init_mint_instr, &accounts[..])?;
                        }
                        Ok(())
                    },
                )?;
            }

            // Update the vesting account.
            vesting_account.locked_outstanding += lsrm_nft_count;

            Ok(())
        },
    )
    .map_err(|e| SafeError::ProgramError(e))
}

fn mint_locked_srm_access_control(
    vesting_account: &VestingAccount,
    vesting_account_beneficiary: &AccountInfo,
    lsrm_nfts: &[(AccountInfo, &AccountInfo)], // (spl mint, lsrm receipt) pairs
    lsrm_nft_count: u64,
    spl_token_program_account_info: &AccountInfo,
    rent: &Rent,
    rent_account_info: &AccountInfo,
) -> Result<(), SafeError> {
    assert_eq!(*spl_token_program_account_info.key, spl_token::ID);
    assert_eq!(*rent_account_info.key, solana_sdk::sysvar::rent::id());

    if !vesting_account_beneficiary.is_signer {
        return Err(SafeError::ErrorCode(SafeErrorCode::Unauthorized));
    }
    if *vesting_account_beneficiary.key != vesting_account.beneficiary {
        return Err(SafeError::ErrorCode(SafeErrorCode::Unauthorized));
    }
    if vesting_account.total() - vesting_account.locked_outstanding < lsrm_nft_count {
        return Err(SafeError::ErrorCode(SafeErrorCode::InsufficientBalance));
    }

    // Perform checks on all NFT instances.
    for nft in lsrm_nfts {
        let (mint, receipt) = nft;

        // NFT mint must be uninitialized.
        let data = mint.try_borrow_data()?;
        let is_initialized = data[0x2d];
        if is_initialized != 0u8 {
            return Err(SafeError::ErrorCode(SafeErrorCode::LsrmMintAlreadyInitialized).into());
        }

        // LsrmReceipt must be uninitialized.
        let data = receipt.try_borrow_data()?;
        let initialized = data[0];
        if initialized != 0u8 {
            return Err(SafeError::ErrorCode(SafeErrorCode::LsrmReceiptAlreadyInitialized).into());
        }
    }
    Ok(())
}

pub fn burn_locked_srm(accounts: &[AccountInfo]) -> Result<(), SafeError> {
    info!("**********burn SRM!");
    Ok(())
}

pub fn withdraw_srm(accounts: &[AccountInfo], amount: u64) -> Result<(), SafeError> {
    info!("**********withdraw SRM!");
    Ok(())
}

pub fn slash(accounts: &[AccountInfo], amount: u64) -> Result<(), SafeError> {
    // todo
    Ok(())
}

pub fn whitelist_add(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    program_id_to_add: Pubkey,
) -> Result<(), SafeError> {
    info!("HANDLER: whitelist_add");

    let account_info_iter = &mut accounts.iter();

    let safe_authority_info = next_account_info(account_info_iter)?;
    let safe_account_info = next_account_info(account_info_iter)?;

    let mut safe_account_data = safe_account_info.data.borrow_mut();
    SafeAccount::unpack_unchecked_mut(
        &mut safe_account_data,
        &mut |safe_account: &mut SafeAccount| {
            whitelist_add_access_control(
                program_id,
                safe_authority_info,
                safe_account,
                safe_account_info,
            )?;

            if safe_account.whitelist.push(program_id_to_add).is_none() {
                return Err(SafeError::ErrorCode(SafeErrorCode::WhitelistFull).into());
            }

            info!("whitelist_add complete");

            Ok(())
        },
    )
    .map_err(|e| SafeError::ProgramError(e))
}

fn whitelist_add_access_control(
    program_id: &Pubkey,
    safe_authority_info: &AccountInfo,
    safe_account: &SafeAccount,
    safe_account_info: &AccountInfo,
) -> Result<(), ProgramError> {
    if !safe_account.is_initialized {
        return Err(SafeError::ErrorCode(SafeErrorCode::NotInitialized).into());
    }
    if safe_account_info.owner != program_id {
        return Err(SafeError::ErrorCode(SafeErrorCode::NotOwnedByProgram).into());
    }
    if *safe_authority_info.key != safe_account.authority {
        return Err(SafeError::ErrorCode(SafeErrorCode::NotSignedByAuthority).into());
    }
    if !safe_authority_info.is_signer {
        return Err(SafeError::ErrorCode(SafeErrorCode::NotSignedByAuthority).into());
    }
    // TODO.
    Ok(())
}

pub fn whitelist_delete(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    program_id_to_delete: Pubkey,
) -> Result<(), SafeError> {
    info!("HANDLER: whitelist_delete");

    let account_info_iter = &mut accounts.iter();

    let safe_authority_info = next_account_info(account_info_iter)?;
    let safe_account_info = next_account_info(account_info_iter)?;

    let mut safe_account_data = safe_account_info.data.borrow_mut();

    SafeAccount::unpack_unchecked_mut(
        &mut safe_account_data,
        &mut |safe_account: &mut SafeAccount| {
            whitelist_delete_access_control(
                program_id,
                safe_authority_info,
                safe_account,
                safe_account_info,
            )?;

            if safe_account
                .whitelist
                .delete(program_id_to_delete)
                .is_none()
            {
                return Err(SafeError::ErrorCode(SafeErrorCode::WhitelistEntryNotFound).into());
            }

            info!("whitelist_delete complete");

            Ok(())
        },
    )
    .map_err(|e| SafeError::ProgramError(e))
}

fn whitelist_delete_access_control(
    program_id: &Pubkey,
    safe_authority_info: &AccountInfo,
    safe_account: &SafeAccount,
    safe_account_info: &AccountInfo,
) -> Result<(), ProgramError> {
    whitelist_add_access_control(
        program_id,
        safe_authority_info,
        safe_account,
        safe_account_info,
    )
}
