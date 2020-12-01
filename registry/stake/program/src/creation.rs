use crate::get_basket::stake_simple_basket;
use serum_pool::context::{PoolContext, UserAccounts};
use serum_pool_schema::{Basket, PoolState};
use serum_stake::accounts::vault;
use serum_stake::error::{StakeError, StakeErrorCode};
use solana_program::info;
use solana_sdk::pubkey::Pubkey;
use spl_token::instruction as token_instruction;
use std::convert::TryInto;

pub fn handler(
    ctx: &PoolContext,
    state: &mut PoolState,
    spt_amount: u64,
) -> Result<(), StakeError> {
    info!("handler: creation");

    let &UserAccounts {
        pool_token_account,                         // Owned by Registry.
        asset_accounts: registry_deposit_acc_infos, // Registry deposit vaults.
        authority: registry_signer_acc_info,        // Registry's program-derived address.
    } = ctx
        .user_accounts
        .as_ref()
        .expect("transact requests have user accounts");

    assert!(ctx.custom_accounts.len() == 0);
    assert!(registry_deposit_acc_infos.len() == 1 || registry_deposit_acc_infos.len() == 2);
    assert!(ctx.pool_vault_accounts.len() == registry_deposit_acc_infos.len());

    // Auth.
    if !registry_signer_acc_info.is_signer {
        return Err(StakeErrorCode::Unauthorized)?;
    }
    let expected_admin: Pubkey = state.admin_key.clone().expect("must have admin key").into();
    if expected_admin != *registry_signer_acc_info.key {
        return Err(StakeErrorCode::Unauthorized)?;
    }

    // Quantities needed to create the `spt_amount` of staking pool tokens.
    let basket = stake_simple_basket(ctx, state, spt_amount, true)?;

    // Transfer the SRM *into* the pool.
    {
        let escrow_acc_info = &registry_deposit_acc_infos[0];
        let pool_token_vault_acc_info = &ctx.pool_vault_accounts[0];
        let asset_amount = basket.quantities[0]
            .try_into()
            .map_err(|_| StakeErrorCode::FailedCast)?;
        let transfer_instr = token_instruction::transfer(
            &spl_token::ID,
            escrow_acc_info.key,
            pool_token_vault_acc_info.key,
            registry_signer_acc_info.key,
            &[],
            asset_amount,
        )?;
        solana_sdk::program::invoke(
            &transfer_instr,
            &[
                escrow_acc_info.clone(),
                pool_token_vault_acc_info.clone(),
                registry_signer_acc_info.clone(),
                ctx.spl_token_program.expect("must be provided").clone(),
            ],
        )?;
    }

    // Transfer the MSRM *into* the pool, if this is indeed the MSRM pool.
    if registry_deposit_acc_infos.len() == 2 {
        let escrow_acc_info = &registry_deposit_acc_infos[1];
        let pool_token_vault_acc_info = &ctx.pool_vault_accounts[1];
        let asset_amount = basket.quantities[1]
            .try_into()
            .map_err(|_| StakeErrorCode::FailedCast)?;
        let transfer_instr = token_instruction::transfer(
            &spl_token::ID,
            escrow_acc_info.key,
            pool_token_vault_acc_info.key,
            registry_signer_acc_info.key,
            &[],
            asset_amount,
        )?;
        solana_sdk::program::invoke(
            &transfer_instr,
            &[
                escrow_acc_info.clone(),
                pool_token_vault_acc_info.clone(),
                registry_signer_acc_info.clone(),
                ctx.spl_token_program.expect("must be provided").clone(),
            ],
        )?;
    }

    // Mint `spt_amount` of staking pool tokens to the Registry's beneficiary.
    {
        let signer_seeds = vault::signer_seeds(ctx.pool_account.key, &state.vault_signer_nonce);
        let mint_tokens_instr = token_instruction::mint_to(
            &spl_token::ID,
            ctx.pool_token_mint.key,
            pool_token_account.key,
            ctx.pool_authority.key,
            &[],
            spt_amount,
        )?;
        solana_sdk::program::invoke_signed(
            &mint_tokens_instr,
            &[
                ctx.pool_token_mint.clone(),
                pool_token_account.clone(),
                ctx.pool_authority.clone(),
                ctx.spl_token_program.expect("must be provided").clone(),
            ],
            &[&signer_seeds],
        )?;
    }

    Ok(())
}
