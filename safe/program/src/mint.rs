use serum_common::pack::Pack;
use serum_safe::accounts::{MintReceipt, TokenVault, Vesting};
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
    token_acc_owner: Pubkey,
) -> Result<(), SafeError> {
    info!("handler: mint_locked");

    let acc_infos = &mut accounts.iter();

    let vesting_acc_beneficiary_info = next_account_info(acc_infos)?;
    let vesting_acc_info = next_account_info(acc_infos)?;
    let safe_acc_info = next_account_info(acc_infos)?;
    let safe_vault_authority_acc_info = next_account_info(acc_infos)?;
    let token_program_acc_info = next_account_info(acc_infos)?;
    let rent_acc_info = next_account_info(acc_infos)?;
    let mint_acc_info = next_account_info(acc_infos)?;
    let token_acc_info = next_account_info(acc_infos)?;
    let receipt_acc_info = next_account_info(acc_infos)?;

    access_control(AccessControlRequest {
        program_id,
        vesting_acc_info,
        vesting_acc_beneficiary_info,
        token_program_acc_info,
        rent_acc_info,
        mint_acc_info,
        token_acc_info,
        receipt_acc_info,
    })?;

    Vesting::unpack_mut(
        &mut vesting_acc_info.try_borrow_mut_data()?,
        &mut |vesting_acc: &mut Vesting| {
            MintReceipt::unpack_mut(
                &mut receipt_acc_info.try_borrow_mut_data()?,
                &mut |receipt: &mut MintReceipt| {
                    state_transition(StateTransitionRequest {
                        accounts,
                        vesting_acc_info,
                        vesting_acc,
                        token_acc_owner,
                        safe_acc_info,
                        safe_vault_authority_acc_info,
                        mint_acc_info,
                        token_acc_info,
                        receipt,
                    })
                    .map_err(Into::into)
                },
            )
        },
    )?;

    Ok(())
}

fn access_control<'a>(req: AccessControlRequest<'a>) -> Result<(), SafeError> {
    info!("access-control: mint");

    let AccessControlRequest {
        program_id,
        vesting_acc_info,
        vesting_acc_beneficiary_info,
        token_program_acc_info,
        rent_acc_info,
        mint_acc_info,
        token_acc_info,
        receipt_acc_info,
    } = req;

    // Beneficiary authorization.
    {
        if !vesting_acc_beneficiary_info.is_signer {
            return Err(SafeErrorCode::Unauthorized)?;
        }
    }

    // Vesting.
    {
        let vesting = Vesting::unpack(&vesting_acc_info.try_borrow_data()?)?;

        if vesting_acc_info.owner != program_id {
            return Err(SafeErrorCode::InvalidAccount)?;
        }
        if !vesting.initialized {
            return Err(SafeErrorCode::NotInitialized)?;
        }
        // Match the signing beneficiary to this account.
        if vesting.beneficiary != *vesting_acc_beneficiary_info.key {
            return Err(SafeErrorCode::Unauthorized)?;
        }
        // Do we have sufficient balance?
        if vesting.available_for_mint() < 1 {
            return Err(SafeErrorCode::InsufficientBalance)?;
        }
    }

    let rent = Rent::from_account_info(rent_acc_info)?;

    // Mint.
    {
        let mint = spl_token::state::Mint::unpack_unchecked(&mint_acc_info.try_borrow_data()?)?;
        if mint.is_initialized {
            return Err(SafeErrorCode::MintAlreadyInitialized)?;
        }
        if *mint_acc_info.owner != spl_token::ID {
            return Err(SafeErrorCode::InvalidMint)?;
        }
        if !rent.is_exempt(mint_acc_info.lamports(), mint_acc_info.try_data_len()?) {
            return Err(SafeErrorCode::NotRentExempt)?;
        }
    }
    // Token account.
    {
        let token_acc =
            spl_token::state::Account::unpack_unchecked(&token_acc_info.try_borrow_data()?)?;
        if token_acc.state != spl_token::state::AccountState::Uninitialized {
            return Err(SafeErrorCode::TokenAccountAlreadyInitialized)?;
        }
        if *token_acc_info.owner != spl_token::ID {
            return Err(SafeErrorCode::InvalidAccountOwner)?;
        }
        if !rent.is_exempt(token_acc_info.lamports(), token_acc_info.try_data_len()?) {
            return Err(SafeErrorCode::NotRentExempt)?;
        }
    }
    // Receipt.
    {
        let receipt = MintReceipt::unpack(&receipt_acc_info.try_borrow_data()?)?;
        if receipt.initialized {
            return Err(SafeErrorCode::ReceiptAlreadyInitialized)?;
        }
        if receipt_acc_info.owner != program_id {
            return Err(SafeErrorCode::InvalidAccountOwner)?;
        }
        if !rent.is_exempt(
            receipt_acc_info.lamports(),
            receipt_acc_info.try_data_len()?,
        ) {
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

    info!("access-control: success");

    Ok(())
}

fn state_transition<'a, 'b>(req: StateTransitionRequest<'a, 'b>) -> Result<(), SafeError> {
    info!("state-transition: mint");

    let StateTransitionRequest {
        accounts,
        vesting_acc_info,
        token_acc_owner,
        safe_acc_info,
        safe_vault_authority_acc_info,
        mint_acc_info,
        token_acc_info,
        vesting_acc,
        receipt,
    } = req;

    // Initialize the receipt.
    {
        receipt.initialized = true;
        receipt.mint = *mint_acc_info.key;
        receipt.token_acc = *token_acc_info.key;
        receipt.vesting_acc = *vesting_acc_info.key;
        receipt.burned = false;
    }
    // Initialize the NFT mint.
    {
        info!("invoke: spl_token::instruction::initialize_mint");
        let init_mint_instr = spl_token::instruction::initialize_mint(
            &spl_token::ID,
            &mint_acc_info.key,
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
            token_acc_info.key,
            &mint_acc_info.key,
            &token_acc_owner,
        )?;
        solana_sdk::program::invoke(&init_acc_instr, &accounts[..])?;
    }
    // Mint the one and only supply to the NFT holding account.
    {
        info!("invoke: spl_token::instruction::mint_to");

        let mint_to_instr = spl_token::instruction::mint_to(
            &spl_token::ID,
            mint_acc_info.key,
            token_acc_info.key,
            safe_vault_authority_acc_info.key,
            &[],
            1,
        )?;

        let data = safe_acc_info.try_borrow_data()?;
        let nonce = data[data.len() - 1];
        let signer_seeds = TokenVault::signer_seeds(safe_acc_info.key, &nonce);

        solana_sdk::program::invoke_signed(&mint_to_instr, &accounts[..], &[&signer_seeds])?;
    }
    // Set the mint authority to None.
    {
        info!("invoke: spl_token::instruction::set_authority");
        let set_authority_instr = spl_token::instruction::set_authority(
            &spl_token::ID,
            &mint_acc_info.key,
            None,
            spl_token::instruction::AuthorityType::MintTokens,
            safe_vault_authority_acc_info.key,
            &[],
        )?;

        let data = safe_acc_info.try_borrow_data()?;
        let nonce = data[data.len() - 1];
        let signer_seeds = TokenVault::signer_seeds(safe_acc_info.key, &nonce);

        solana_sdk::program::invoke_signed(&set_authority_instr, &accounts[..], &[&signer_seeds])?;
    }

    // Update the vesting account.
    vesting_acc.locked_outstanding += 1;

    info!("state-transition: success");

    Ok(())
}

struct AccessControlRequest<'a> {
    program_id: &'a Pubkey,
    vesting_acc_info: &'a AccountInfo<'a>,
    vesting_acc_beneficiary_info: &'a AccountInfo<'a>,
    token_program_acc_info: &'a AccountInfo<'a>,
    rent_acc_info: &'a AccountInfo<'a>,
    mint_acc_info: &'a AccountInfo<'a>,
    token_acc_info: &'a AccountInfo<'a>,
    receipt_acc_info: &'a AccountInfo<'a>,
}

struct StateTransitionRequest<'a, 'b> {
    accounts: &'a [AccountInfo<'a>],
    vesting_acc_info: &'a AccountInfo<'a>,
    token_acc_owner: Pubkey,
    safe_acc_info: &'a AccountInfo<'a>,
    safe_vault_authority_acc_info: &'a AccountInfo<'a>,
    mint_acc_info: &'a AccountInfo<'a>,
    token_acc_info: &'a AccountInfo<'a>,
    vesting_acc: &'b mut Vesting,
    receipt: &'b mut MintReceipt,
}
