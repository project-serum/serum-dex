use serum_common::pack::Pack;
use serum_safe::accounts::{LsrmReceipt, SrmVault, VestingAccount};
use serum_safe::error::{SafeError, SafeErrorCode};
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::info;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::sysvar::rent::Rent;
use solana_sdk::sysvar::Sysvar;
use spl_token::pack::Pack as TokenPack;

pub fn handler<'a>(
    _program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'a>],
    token_acc_owner: Pubkey,
) -> Result<(), SafeError> {
    info!("handler: mint_locked_srm");

    let acc_infos = &mut accounts.iter();

    let vesting_acc_beneficiary_info = next_account_info(acc_infos)?;
    let vesting_acc_info = next_account_info(acc_infos)?;
    let safe_acc_info = next_account_info(acc_infos)?;
    let safe_vault_authority_acc_info = next_account_info(acc_infos)?;
    let spl_token_program_acc_info = next_account_info(acc_infos)?;
    let rent_acc_info = next_account_info(acc_infos)?;
    let lsrm_nfts = {
        let lsrm_nft_count = ((accounts.len() - FIXED_ACCS) / DYN_ACC_PIECES) as u64;
        let mut lsrm_nfts = vec![];
        for _ in 0..lsrm_nft_count {
            let lsrm_spl_mint_info = next_account_info(acc_infos)?;
            let lsrm_token_acc_info = next_account_info(acc_infos)?;
            let lsrm_receipt_info = next_account_info(acc_infos)?;
            lsrm_nfts.push((
                lsrm_spl_mint_info.clone(),
                lsrm_token_acc_info,
                lsrm_receipt_info,
            ));
        }
        lsrm_nfts
    };

    access_control(AccessControlRequest {
        vesting_acc_info,
        vesting_acc_beneficiary_info,
        lsrm_nfts: &lsrm_nfts,
        spl_token_program_acc_info,
        rent_acc_info,
        accounts_len: accounts.len(),
    })?;

    VestingAccount::unpack_mut(
        &mut vesting_acc_info.try_borrow_mut_data()?,
        &mut |vesting_acc: &mut VestingAccount| {
            state_transition(StateTransitionRequest {
                accounts,
                lsrm_nfts: &lsrm_nfts,
                vesting_acc_info,
                vesting_acc,
                token_acc_owner,
                safe_acc_info,
                safe_vault_authority_acc_info,
            })?;
            Ok(())
        },
    )
    .map_err(|e| SafeError::ProgramError(e))
}

fn access_control<'a, 'b>(req: AccessControlRequest<'a, 'b>) -> Result<(), SafeError> {
    info!("access-control: mint");

    let AccessControlRequest {
        vesting_acc_info,
        vesting_acc_beneficiary_info,
        lsrm_nfts,
        spl_token_program_acc_info,
        rent_acc_info,
        accounts_len,
    } = req;

    if (accounts_len - FIXED_ACCS) % DYN_ACC_PIECES != 0 {
        return Err(SafeError::ErrorCode(SafeErrorCode::WrongNumberOfAccounts));
    }

    assert_eq!(*spl_token_program_acc_info.key, spl_token::ID);
    assert_eq!(*rent_acc_info.key, solana_sdk::sysvar::rent::id());

    if !vesting_acc_beneficiary_info.is_signer {
        return Err(SafeError::ErrorCode(SafeErrorCode::Unauthorized));
    }
    let vesting_acc = VestingAccount::unpack(&vesting_acc_info.try_borrow_data()?)?;
    if *vesting_acc_beneficiary_info.key != vesting_acc.beneficiary {
        return Err(SafeError::ErrorCode(SafeErrorCode::Unauthorized));
    }
    let lsrm_nft_count = ((accounts_len - FIXED_ACCS) / DYN_ACC_PIECES) as u64;
    if vesting_acc.total() - vesting_acc.locked_outstanding < lsrm_nft_count {
        return Err(SafeError::ErrorCode(SafeErrorCode::InsufficientBalance));
    }

    // Perform checks on all NFT instances.
    for (mint, token_acc, receipt) in lsrm_nfts {
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
        let rent = Rent::from_account_info(rent_acc_info)?;
        if !rent.is_exempt(mint.lamports(), nft_data_len) {
            return Err(SafeError::ErrorCode(SafeErrorCode::NotRentExempt).into());
        }
        if !rent.is_exempt(receipt.lamports(), receipt_data_len) {
            return Err(SafeError::ErrorCode(SafeErrorCode::NotRentExempt).into());
        }
        if !rent.is_exempt(token_acc.lamports(), token_acc.try_data_len()?) {
            return Err(SafeError::ErrorCode(SafeErrorCode::NotRentExempt).into());
        }
        // Token account must be uninitialized.
        let token_acc = spl_token::state::Account::unpack_unchecked(&token_acc.try_borrow_data()?)?;
        if token_acc.state != spl_token::state::AccountState::Uninitialized {
            return Err(SafeError::ErrorCode(
                SafeErrorCode::TokenAccountAlreadyInitialized,
            ));
        }
    }
    info!("access-control: success");

    Ok(())
}

fn state_transition<'a, 'b>(req: StateTransitionRequest<'a, 'b>) -> Result<(), SafeError> {
    info!("state-transition: mint");

    let StateTransitionRequest {
        accounts,
        lsrm_nfts,
        vesting_acc_info,
        vesting_acc,
        token_acc_owner,
        safe_acc_info,
        safe_vault_authority_acc_info,
    } = req;

    // Initialize all receipts, mints, and token accounts.
    for (mint, token_acc, receipt) in lsrm_nfts {
        LsrmReceipt::unpack_mut(
            &mut receipt.try_borrow_mut_data()?,
            &mut |receipt: &mut LsrmReceipt| {
                // Initialize the receipt.
                {
                    receipt.initialized = true;
                    receipt.mint = *mint.key;
                    receipt.spl_account = *token_acc.key;
                    receipt.vesting_account = *vesting_acc_info.key;
                    receipt.burned = false;
                }
                // Initialize the NFT mint.
                {
                    info!("invoke: spl_token::instruction::initialize_mint");
                    let init_mint_instr = spl_token::instruction::initialize_mint(
                        &spl_token::ID,
                        &mint.key,
                        &safe_vault_authority_acc_info.key,
                        None,
                        0,
                    )?;
                    solana_sdk::program::invoke(&init_mint_instr, &accounts[..])?;
                }
                // Initialize the NFT holding account.
                {
                    info!("invoke: spl_token::instruction::initialize_account");
                    let init_acc_instr = spl_token::instruction::initialize_account(
                        &spl_token::ID,
                        token_acc.key,
                        &mint.key,
                        &token_acc_owner,
                    )?;
                    solana_sdk::program::invoke(&init_acc_instr, &accounts[..])?;
                }
                // Mint the one and only supply to the NFT holding account.
                {
                    info!("invoke: spl_token::instruction::mint_to");

                    let mint_to_instr = spl_token::instruction::mint_to(
                        &spl_token::ID,
                        mint.key,
                        token_acc.key,
                        safe_vault_authority_acc_info.key,
                        &[],
                        1,
                    )?;

                    let data = safe_acc_info.try_borrow_data()?;
                    let nonce = &[data[data.len() - 1]];
                    let signer_seeds = SrmVault::signer_seeds(safe_acc_info.key, nonce);

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
                        safe_vault_authority_acc_info.key,
                        &[],
                    )?;

                    let data = safe_acc_info.try_borrow_data()?;
                    let nonce = &[data[data.len() - 1]];
                    let signer_seeds = SrmVault::signer_seeds(safe_acc_info.key, nonce);

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
    vesting_acc.locked_outstanding += lsrm_nfts.len() as u64;

    info!("state-transition: success");

    Ok(())
}

struct AccessControlRequest<'a, 'b> {
    vesting_acc_info: &'a AccountInfo<'a>,
    vesting_acc_beneficiary_info: &'a AccountInfo<'a>,
    // (SPL mint, token account, lsrm receipt) pairs.
    lsrm_nfts: &'b [(AccountInfo<'a>, &'a AccountInfo<'a>, &'a AccountInfo<'a>)],
    spl_token_program_acc_info: &'a AccountInfo<'a>,
    rent_acc_info: &'a AccountInfo<'a>,
    accounts_len: usize,
}

struct StateTransitionRequest<'a, 'b> {
    accounts: &'a [AccountInfo<'a>],
    // (SPL mint, token account, lsrm receipt) pairs.
    lsrm_nfts: &'b [(AccountInfo<'a>, &'a AccountInfo<'a>, &'a AccountInfo<'a>)],
    vesting_acc_info: &'a AccountInfo<'a>,
    vesting_acc: &'b mut VestingAccount,
    token_acc_owner: Pubkey,
    safe_acc_info: &'a AccountInfo<'a>,
    safe_vault_authority_acc_info: &'a AccountInfo<'a>,
}

// The number of accounts that are non-variable in the instruction.
const FIXED_ACCS: usize = 6;
// The pair size for accounts that are viariable in this instruction.
// That is, every lSRM has `dyn_account_pieces` number of accounts
// associated with it.
const DYN_ACC_PIECES: usize = 3;
