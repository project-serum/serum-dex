use serum_common::pack::DynPack;
use serum_safe::accounts::{LsrmReceipt, SrmVault, VestingAccount};
use serum_safe::error::{SafeError, SafeErrorCode};
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::info;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::sysvar::rent::Rent;
use solana_sdk::sysvar::Sysvar;
use spl_token::pack::Pack;

pub fn handler<'a>(
    _program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'a>],
    token_account_owner: Pubkey,
) -> Result<(), SafeError> {
    info!("handler: mint_locked_srm");

    let fixed_accounts = 6;
    let dyn_account_pieces = 3;

    let accounts_len = accounts.len();
    if (accounts_len - fixed_accounts) % dyn_account_pieces != 0 {
        return Err(SafeError::ErrorCode(SafeErrorCode::WrongNumberOfAccounts));
    }
    let lsrm_nft_count = ((accounts_len - fixed_accounts) / dyn_account_pieces) as u64;

    let account_info_iter = &mut accounts.iter();

    let vesting_account_beneficiary_info = next_account_info(account_info_iter)?;
    let vesting_account_info = next_account_info(account_info_iter)?;
    let safe_account_info = next_account_info(account_info_iter)?;
    let safe_vault_authority_account_info = next_account_info(account_info_iter)?;
    let spl_token_program_account_info = next_account_info(account_info_iter)?;

    let rent_account_info = next_account_info(account_info_iter)?;
    let rent = Rent::from_account_info(rent_account_info)?;

    let mut lsrm_nfts = vec![];

    for _ in 0..lsrm_nft_count {
        let lsrm_spl_mint_info = next_account_info(account_info_iter)?;
        let lsrm_token_account_info = next_account_info(account_info_iter)?;
        let lsrm_receipt_info = next_account_info(account_info_iter)?;
        lsrm_nfts.push((
            lsrm_spl_mint_info.clone(),
            lsrm_token_account_info,
            lsrm_receipt_info,
        ));
    }

    VestingAccount::unpack_mut(
        &mut vesting_account_info.try_borrow_mut_data()?,
        &mut |vesting_account: &mut VestingAccount| {
            access_control(AccessControlRequest {
                vesting_account,
                vesting_account_beneficiary_info,
                lsrm_nfts: &lsrm_nfts,
                lsrm_nft_count,
                spl_token_program_account_info,
                rent,
                rent_account_info,
            })?;

            state_transition(StateTransitionRequest {
                accounts,
                lsrm_nfts: &lsrm_nfts,
                vesting_account_info,
                vesting_account,
                lsrm_nft_count,
                token_account_owner,
                safe_account_info,
                safe_vault_authority_account_info,
            })?;

            Ok(())
        },
    )
    .map_err(|e| SafeError::ProgramError(e))
}

fn access_control<'a, 'b>(req: AccessControlRequest<'a, 'b>) -> Result<(), SafeError> {
    info!("access-control: mint");

    let AccessControlRequest {
        vesting_account,
        vesting_account_beneficiary_info,
        lsrm_nfts,
        lsrm_nft_count,
        spl_token_program_account_info,
        rent,
        rent_account_info,
    } = req;

    assert_eq!(*spl_token_program_account_info.key, spl_token::ID);
    assert_eq!(*rent_account_info.key, solana_sdk::sysvar::rent::id());

    if !vesting_account_beneficiary_info.is_signer {
        return Err(SafeError::ErrorCode(SafeErrorCode::Unauthorized));
    }
    if *vesting_account_beneficiary_info.key != vesting_account.beneficiary {
        return Err(SafeError::ErrorCode(SafeErrorCode::Unauthorized));
    }
    if vesting_account.total() - vesting_account.locked_outstanding < lsrm_nft_count {
        return Err(SafeError::ErrorCode(SafeErrorCode::InsufficientBalance));
    }

    // Perform checks on all NFT instances.
    for (mint, token_account, receipt) in lsrm_nfts {
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
        // Rent Exemption.
        if !rent.is_exempt(mint.lamports(), nft_data_len) {
            return Err(SafeError::ErrorCode(SafeErrorCode::NotRentExempt).into());
        }
        if !rent.is_exempt(receipt.lamports(), receipt_data_len) {
            return Err(SafeError::ErrorCode(SafeErrorCode::NotRentExempt).into());
        }
        if !rent.is_exempt(token_account.lamports(), token_account.try_data_len()?) {
            return Err(SafeError::ErrorCode(SafeErrorCode::NotRentExempt).into());
        }
        // Token account must be uninitialized.
        let token_account =
            spl_token::state::Account::unpack_unchecked(&token_account.try_borrow_data()?)?;
        if token_account.state != spl_token::state::AccountState::Uninitialized {
            return Err(SafeError::ErrorCode(
                SafeErrorCode::TokenAccountAlreadyInitialized,
            ));
        }
    }
    info!("access-control: success");

    Ok(())
}

struct AccessControlRequest<'a, 'b> {
    vesting_account: &'b VestingAccount,
    vesting_account_beneficiary_info: &'a AccountInfo<'a>,
    // (SPL mint, token owner, lsrm receipt) pairs.
    lsrm_nfts: &'b [(AccountInfo<'a>, &'a AccountInfo<'a>, &'a AccountInfo<'a>)],
    lsrm_nft_count: u64,
    spl_token_program_account_info: &'a AccountInfo<'a>,
    rent: Rent,
    rent_account_info: &'a AccountInfo<'a>,
}

fn state_transition<'a, 'b>(req: StateTransitionRequest<'a, 'b>) -> Result<(), SafeError> {
    info!("state-transition: mint");

    let StateTransitionRequest {
        accounts,
        lsrm_nfts,
        vesting_account_info,
        vesting_account,
        lsrm_nft_count,
        token_account_owner,
        safe_account_info,
        safe_vault_authority_account_info,
    } = req;

    // Initialize all receipts, mints, and token accounts.
    for (mint, token_account, receipt) in lsrm_nfts {
        LsrmReceipt::unpack_unchecked_mut(
            &mut receipt.try_borrow_mut_data()?,
            &mut |receipt: &mut LsrmReceipt| {
                // Initialize the receipt.
                {
                    receipt.initialized = true;
                    receipt.mint = *mint.key;
                    receipt.spl_account = *token_account.key;
                    receipt.vesting_account = *vesting_account_info.key;
                    receipt.burned = false;
                }
                // Initialize the NFT mint.
                {
                    info!("invoke: spl_token::instruction::initialize_mint");
                    let init_mint_instr = spl_token::instruction::initialize_mint(
                        &spl_token::ID,
                        &mint.key,
                        &safe_vault_authority_account_info.key,
                        None,
                        0,
                    )?;
                    solana_sdk::program::invoke(&init_mint_instr, &accounts[..])?;
                }
                // Initialize the NFT holding account.
                {
                    info!("invoke: spl_token::instruction::initialize_account");
                    let init_account_instr = spl_token::instruction::initialize_account(
                        &spl_token::ID,
                        token_account.key,
                        &mint.key,
                        &token_account_owner,
                    )?;
                    solana_sdk::program::invoke(&init_account_instr, &accounts[..])?;
                }
                // Mint the one and only supply to the NFT holding account.
                {
                    info!("invoke: spl_token::instruction::mint_to");

                    let mint_to_instr = spl_token::instruction::mint_to(
                        &spl_token::ID,
                        mint.key,
                        token_account.key,
                        safe_vault_authority_account_info.key,
                        &[],
                        1,
                    )?;

                    let data = safe_account_info.try_borrow_data()?;
                    let nonce = &[data[data.len() - 1]];
                    let signer_seeds = SrmVault::signer_seeds(safe_account_info.key, nonce);

                    solana_sdk::program::invoke_signed(
                        &mint_to_instr,
                        &accounts[..],
                        &[&signer_seeds],
                    )?;
                }
                // Set the mint authority to None.
                {
                    info!("invoke: spl_token::instruction::set_authority");
                    let set_authority_instr = spl_token::instruction::set_authority(
                        &spl_token::ID,
                        &mint.key,
                        None,
                        spl_token::instruction::AuthorityType::MintTokens,
                        safe_vault_authority_account_info.key,
                        &[],
                    )?;

                    let data = safe_account_info.try_borrow_data()?;
                    let nonce = &[data[data.len() - 1]];
                    let signer_seeds = SrmVault::signer_seeds(safe_account_info.key, nonce);

                    solana_sdk::program::invoke_signed(
                        &set_authority_instr,
                        &accounts[..],
                        &[&signer_seeds],
                    )?;
                }

                Ok(())
            },
        )?;
    }

    // Update the vesting account.
    vesting_account.locked_outstanding += lsrm_nft_count;

    info!("state-transition: success");

    Ok(())
}

struct StateTransitionRequest<'a, 'b> {
    accounts: &'a [AccountInfo<'a>],
    // (spl mint, spl account, lsrm receipt) pairs
    lsrm_nfts: &'b [(AccountInfo<'a>, &'a AccountInfo<'a>, &'a AccountInfo<'a>)],
    vesting_account_info: &'a AccountInfo<'a>,
    vesting_account: &'b mut VestingAccount,
    lsrm_nft_count: u64,
    token_account_owner: Pubkey,
    safe_account_info: &'a AccountInfo<'a>,
    safe_vault_authority_account_info: &'a AccountInfo<'a>,
}
