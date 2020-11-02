use serum_pool::context::{PoolContext, UserAccounts};
use serum_pool_schema::PoolState;
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
    info!("handler: redemption");

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

    // Quantities exchanged when redeeming `spt_amount` staking pool tokens.
    let basket = ctx.get_simple_basket(spt_amount, false)?;

    let signer_seeds = vault::signer_seeds(ctx.pool_account.key, &state.vault_signer_nonce);

    // Transfer out the SRM to the user.
    {
        let escrow_token_acc_info = &registry_deposit_acc_infos[0];
        let vault_acc_info = &ctx.pool_vault_accounts[0];
        let asset_amount = basket.quantities[0]
            .try_into()
            .map_err(|_| StakeErrorCode::InvalidU64)?;
        let transfer_instr = token_instruction::transfer(
            &spl_token::ID,
            vault_acc_info.key,
            escrow_token_acc_info.key,
            ctx.pool_authority.key,
            &[],
            asset_amount,
        )?;
        solana_sdk::program::invoke_signed(
            &transfer_instr,
            &[
                escrow_token_acc_info.clone(),
                vault_acc_info.clone(),
                ctx.pool_authority.clone(),
                ctx.spl_token_program.expect("must be provided").clone(),
            ],
            &[&signer_seeds],
        )?;
    }

    // Transer out the MSRM to the user.
    if registry_deposit_acc_infos.len() == 2 {
        let escrow_token_acc_info = &registry_deposit_acc_infos[1];
        let mega_vault_acc_info = &ctx.pool_vault_accounts[1];
        let asset_amount = basket.quantities[1]
            .try_into()
            .map_err(|_| StakeErrorCode::InvalidU64)?;
        let transfer_instr = token_instruction::transfer(
            &spl_token::ID,
            mega_vault_acc_info.key,
            escrow_token_acc_info.key,
            ctx.pool_authority.key,
            &[],
            asset_amount,
        )?;
        solana_sdk::program::invoke_signed(
            &transfer_instr,
            &[
                escrow_token_acc_info.clone(),
                mega_vault_acc_info.clone(),
                ctx.pool_authority.clone(),
                ctx.spl_token_program.expect("must be provided").clone(),
            ],
            &[&signer_seeds],
        )?;
    }

    // Burn the given `spt_amount` of staking pool tokens.
    {
        let mint_tokens_instr = token_instruction::burn(
            &spl_token::ID,
            pool_token_account.key,
            ctx.pool_token_mint.key,
            registry_signer_acc_info.key,
            &[],
            spt_amount,
        )?;
        solana_sdk::program::invoke_signed(
            &mint_tokens_instr,
            &[
                pool_token_account.clone(),
                ctx.pool_token_mint.clone(),
                registry_signer_acc_info.clone(),
                ctx.spl_token_program.expect("must be provided").clone(),
            ],
            &[&signer_seeds],
        )?;
    }

    Ok(())
}
