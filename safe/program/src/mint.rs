use serum_safe::accounts::{LsrmReceipt, VestingAccount};
use serum_safe::error::{SafeError, SafeErrorCode};
use serum_safe::pack::DynPack;
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::info;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::sysvar::rent::Rent;
use solana_sdk::sysvar::Sysvar;
use spl_token::pack::Pack;

pub fn handler(program_id: &Pubkey, accounts: &[AccountInfo]) -> Result<(), SafeError> {
    info!("HANDLER: mint_locked_srm");

    let accounts_len = accounts.len();
    if (accounts_len - 4) % 2 != 0 {
        return Err(SafeError::ErrorCode(SafeErrorCode::WrongNumberOfAccounts));
    }
    let lsrm_nft_count = ((accounts_len - 4) / 2) as u64;

    let account_info_iter = &mut accounts.iter();

    let vesting_account_beneficiary_info = next_account_info(account_info_iter)?;
    let vesting_account_info = next_account_info(account_info_iter)?;
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
            access_control(
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

fn access_control(
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
