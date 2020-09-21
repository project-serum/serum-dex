//! api.rs defines all instruction handlers for the program.

use arrayref::array_mut_ref;
use safe_transmute::to_bytes::transmute_to_bytes;
use serum_safe::accounts::{LsrmReceipt, SafeAccount, SrmVault, VestingAccount};
use serum_safe::error::{SafeError, SafeErrorCode};
use serum_safe::pack::DynPack;
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::info;
use solana_sdk::program_error::ProgramError;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::sysvar::clock::Clock;
use solana_sdk::sysvar::rent::Rent;
use solana_sdk::sysvar::Sysvar;
use spl_token::pack::{IsInitialized, Pack};

pub fn initialize(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    mint: Pubkey,
    authority: Pubkey,
    nonce: u8,
) -> Result<(), SafeError> {
    info!("HANDLER: initialize");
    let account_info_iter = &mut accounts.iter();
    let safe_account_info = next_account_info(account_info_iter)?;
    let safe_account_data_len = safe_account_info.data_len();
    let rent = &Rent::from_account_info(next_account_info(account_info_iter)?)?;

    let mut safe_account_data = safe_account_info.try_borrow_mut_data()?;
    SafeAccount::unpack_unchecked_mut(
        &mut safe_account_data,
        &mut |safe_account: &mut SafeAccount| {
            initialize_access_control(
                program_id,
                safe_account,
                safe_account_info,
                safe_account_data_len,
                rent,
                nonce,
            )?;

            safe_account.mint = mint;
            safe_account.is_initialized = true;
            safe_account.supply = 0;
            safe_account.authority = authority;
            safe_account.nonce = nonce;
            // todo: consider adding the vault to the safe account directly
            //       if we do that, then check the owner in access control

            info!("safe initialization complete");

            Ok(())
        },
    )
    .map_err(|e| SafeError::ProgramError(e))
}

fn initialize_access_control(
    program_id: &Pubkey,
    safe_account: &SafeAccount,
    safe_account_info: &AccountInfo,
    safe_account_data_len: usize,
    rent: &Rent,
    nonce: u8,
) -> Result<(), SafeError> {
    info!("ACCESS CONTROL: initialize");
    if safe_account.is_initialized {
        return Err(SafeError::ErrorCode(SafeErrorCode::AlreadyInitialized));
    }
    if !rent.is_exempt(safe_account_info.lamports(), safe_account_data_len) {
        return Err(SafeError::ErrorCode(SafeErrorCode::NotRentExempt));
    }
    if safe_account_info.owner != program_id {
        return Err(SafeError::ErrorCode(SafeErrorCode::NotOwnedByProgram));
    }
    if Pubkey::create_program_address(
        &SrmVault::signer_seeds(safe_account_info.key, &[nonce]),
        program_id,
    )
    .is_err()
    {
        return Err(SafeError::ErrorCode(SafeErrorCode::InvalidVaultNonce));
    }
    info!("ACCESS CONTROL: success");
    Ok(())
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
    let nonce = &[safe_account.nonce];
    let seeds = SrmVault::signer_seeds(safe_account_info.key, nonce);
    let expected_authority = Pubkey::create_program_address(&seeds, program_id)?;
    if Pubkey::new(&safe_srm_vault_to.try_borrow_data()?[32..64]) != expected_authority {
        return Err(SafeError::ErrorCode(SafeErrorCode::WrongVault).into());
    }
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
        let nft_data_len = data.len();
        let is_initialized = data[0x2d];
        if is_initialized != 0u8 {
            return Err(SafeError::ErrorCode(SafeErrorCode::LsrmMintAlreadyInitialized).into());
        }
        // LsrmReceipt must be uninitialized.
        let data = receipt.try_borrow_data()?;
        let receipt_data_len = data.len();
        let initialized = data[0];
        if initialized != 0u8 {
            return Err(SafeError::ErrorCode(SafeErrorCode::LsrmReceiptAlreadyInitialized).into());
        }

        // Both must be rent exempt.
        if !rent.is_exempt(mint.lamports(), nft_data_len) {
            return Err(SafeError::ErrorCode(SafeErrorCode::NotRentExempt).into());
        }
        if !rent.is_exempt(receipt.lamports(), receipt_data_len) {
            return Err(SafeError::ErrorCode(SafeErrorCode::NotRentExempt).into());
        }
    }
    Ok(())
}

pub fn burn_locked_srm(accounts: &[AccountInfo]) -> Result<(), SafeError> {
    info!("HANDLER: burn_locked_srm");
    Ok(())
}

pub fn withdraw_srm(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    amount: u64,
) -> Result<(), SafeError> {
    info!("HANDLER: withdraw_srm");

    let account_info_iter = &mut accounts.iter();

    let vesting_account_beneficiary_info = next_account_info(account_info_iter)?;
    let vesting_account_info = next_account_info(account_info_iter)?;
    let beneficiary_spl_account_info = next_account_info(account_info_iter)?;
    let safe_spl_vault_account_info = next_account_info(account_info_iter)?;
    let safe_spl_vault_authority_account_info = next_account_info(account_info_iter)?;
    let safe_account_info = next_account_info(account_info_iter)?;
    let spl_program_account_info = next_account_info(account_info_iter)?;
    let clock = Clock::from_account_info(next_account_info(account_info_iter)?)?;

    VestingAccount::unpack_mut(
        &mut vesting_account_info.try_borrow_mut_data()?,
        &mut |vesting_account: &mut VestingAccount| {
            withdraw_srm_access_control(
                program_id,
                &vesting_account_beneficiary_info,
                vesting_account_info,
                vesting_account,
                amount,
                &clock,
                safe_spl_vault_account_info,
                safe_spl_vault_authority_account_info,
                safe_account_info,
                spl_program_account_info,
            )?;

            vesting_account.deduct(amount);

            info!("invoking withdrawal token transfer");
            let withdraw_instruction = spl_token::instruction::transfer(
                &spl_token::ID,
                safe_spl_vault_account_info.key,
                beneficiary_spl_account_info.key,
                &safe_spl_vault_authority_account_info.key,
                &[],
                amount,
            )?;

            let data = safe_account_info.try_borrow_data()?;
            let nonce = &[data[data.len() - 1]];
            let signer_seeds = SrmVault::signer_seeds(safe_account_info.key, nonce);

            let r = solana_sdk::program::invoke_signed(
                &withdraw_instruction,
                &[
                    safe_spl_vault_account_info.clone(),
                    beneficiary_spl_account_info.clone(),
                    safe_spl_vault_authority_account_info.clone(),
                    spl_program_account_info.clone(),
                ],
                &[&signer_seeds],
            );
            info!("withdrawal token transfer complete");
            r
        },
    )
    .map_err(|e| SafeError::ProgramError(e))
}

pub fn withdraw_srm_access_control(
    program_id: &Pubkey,
    vesting_account_beneficiary_info: &AccountInfo,
    vesting_account_info: &AccountInfo,
    vesting_account: &VestingAccount,
    amount: u64,
    clock: &Clock,
    safe_spl_vault_account_info: &AccountInfo,
    safe_spl_vault_account_authority: &AccountInfo,
    safe_account_info: &AccountInfo,
    spl_program_account_info: &AccountInfo,
) -> Result<(), SafeError> {
    assert_eq!(*spl_program_account_info.key, spl_token::ID);

    if vesting_account_info.owner != program_id {
        return Err(SafeError::ErrorCode(SafeErrorCode::InvalidAccount));
    }
    if !vesting_account_beneficiary_info.is_signer {
        return Err(SafeError::ErrorCode(SafeErrorCode::Unauthorized));
    }
    if vesting_account.beneficiary != *vesting_account_beneficiary_info.key {
        return Err(SafeError::ErrorCode(SafeErrorCode::Unauthorized));
    }
    if amount > vesting_account.available_for_withdrawal(clock.slot) {
        return Err(SafeError::ErrorCode(SafeErrorCode::InsufficientBalance));
    }

    // Validate the vault spl account.
    {
        let spl_vault_data = safe_spl_vault_account_info.try_borrow_data()?;

        assert_eq!(*safe_spl_vault_account_info.owner, spl_token::ID);
        assert_eq!(spl_vault_data.len(), spl_token::state::Account::LEN);

        // AccountState must be initialized.
        if spl_vault_data[0x6c] != 1u8 {
            return Err(SafeError::ErrorCode(SafeErrorCode::WrongVault));
        }
        // The SPL account owner must be hte program derived address.
        let expected_owner = {
            let data = safe_account_info.try_borrow_data()?;
            let nonce = &[data[data.len() - 1]];
            let signer_seeds = SrmVault::signer_seeds(safe_account_info.key, nonce);

            Pubkey::create_program_address(&signer_seeds, program_id)
                .expect("safe initialized with invalid nonce")
        };
        let owner = Pubkey::new(&spl_vault_data[32..64]);
        if owner != expected_owner {
            return Err(SafeError::ErrorCode(SafeErrorCode::WrongVault));
        }
    }
    // todo: check beneficiary account is initialized
    Ok(())
}

pub fn slash(accounts: &[AccountInfo], amount: u64) -> Result<(), SafeError> {
    // todo
    Ok(())
}
