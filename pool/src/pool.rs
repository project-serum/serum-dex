use std::convert::TryInto;

use solana_sdk::{
    account_info::AccountInfo, entrypoint::ProgramResult, program, program_error::ProgramError,
    pubkey::Pubkey,
};

use serum_pool_schema::{Basket, PoolState};

use crate::context::PoolContext;

/// Trait to implement for custom pool implementations.
pub trait Pool {
    #[allow(unused_variables)]
    fn initialize_pool(context: &PoolContext, state: &mut PoolState) -> Result<(), ProgramError> {
        Ok(())
    }

    #[allow(unused_variables)]
    fn get_creation_basket(
        context: &PoolContext,
        state: &PoolState,
        creation_size: u64,
    ) -> Result<Basket, ProgramError> {
        context.get_simple_basket(creation_size, true)
    }

    #[allow(unused_variables)]
    fn get_redemption_basket(
        context: &PoolContext,
        state: &PoolState,
        redemption_size: u64,
    ) -> Result<Basket, ProgramError> {
        context.get_simple_basket(redemption_size, false)
    }

    #[allow(unused_variables)]
    fn get_swap_basket(
        context: &PoolContext,
        state: &PoolState,
        request: &[u64],
    ) -> Result<Basket, ProgramError> {
        return Err(ProgramError::InvalidArgument);
    }

    fn process_creation(
        context: &PoolContext,
        state: &mut PoolState,
        creation_size: u64,
    ) -> Result<(), ProgramError> {
        let basket = Self::get_creation_basket(context, state, creation_size)?;
        let user_accounts = context
            .user_accounts
            .as_ref()
            .ok_or(ProgramError::InvalidArgument)?;
        let pool_vault_accounts = context.pool_vault_accounts;

        let spl_token_program = context
            .spl_token_program
            .ok_or(ProgramError::InvalidArgument)?;

        let zipped_iter = basket
            .quantities
            .iter()
            .zip(user_accounts.asset_accounts.iter())
            .zip(pool_vault_accounts.iter());

        // pull in components
        for ((&input_qty, user_asset_account), pool_vault_account) in zipped_iter {
            let source_pubkey = user_asset_account.key;
            let destination_pubkey = pool_vault_account.key;
            let authority_pubkey = user_accounts.authority.key;
            let signer_pubkeys = &[];

            let instruction = spl_token::instruction::transfer(
                &spl_token::ID,
                source_pubkey,
                destination_pubkey,
                authority_pubkey,
                signer_pubkeys,
                input_qty
                    .try_into()
                    .or(Err(ProgramError::InvalidArgument))?,
            )?;

            let account_infos = &[
                user_asset_account.clone(),
                pool_vault_account.clone(),
                user_accounts.authority.clone(),
                spl_token_program.clone(),
            ];

            program::invoke(&instruction, account_infos)?;
        }

        // push out shares
        context.mint_tokens(state, creation_size)?;

        Ok(())
    }

    fn process_redemption(
        context: &PoolContext,
        state: &mut PoolState,
        redemption_size: u64,
    ) -> Result<(), ProgramError> {
        let redemption_size = context.burn_and_collect_fees(state, redemption_size)?;

        let basket = Self::get_redemption_basket(context, state, redemption_size)?;

        let user_accounts = context
            .user_accounts
            .as_ref()
            .ok_or(ProgramError::InvalidArgument)?;
        let pool_vault_accounts = context.pool_vault_accounts;
        let spl_token_program = context
            .spl_token_program
            .ok_or(ProgramError::InvalidArgument)?;
        let zipped_iter = basket
            .quantities
            .iter()
            .zip(user_accounts.asset_accounts.iter())
            .zip(pool_vault_accounts.iter());

        // push out components
        for ((&output_qty, user_asset_account), pool_vault_account) in zipped_iter {
            let source_pubkey = pool_vault_account.key;
            let destination_pubkey = user_asset_account.key;
            let authority_pubkey = context.pool_authority.key;
            let signer_pubkeys = &[];

            let instruction = spl_token::instruction::transfer(
                &spl_token::ID,
                source_pubkey,
                destination_pubkey,
                authority_pubkey,
                signer_pubkeys,
                output_qty
                    .try_into()
                    .or(Err(ProgramError::InvalidArgument))?,
            )?;

            let account_infos = &[
                user_asset_account.clone(),
                pool_vault_account.clone(),
                context.pool_authority.clone(),
                spl_token_program.clone(),
            ];

            program::invoke_signed(
                &instruction,
                account_infos,
                &[&[
                    context.pool_account.key.as_ref(),
                    &[state.vault_signer_nonce],
                ]],
            )?;
        }
        Ok(())
    }

    #[allow(unused_variables)]
    fn process_swap(
        context: &PoolContext,
        state: &mut PoolState,
        request: &[u64],
    ) -> Result<(), ProgramError> {
        return Err(ProgramError::InvalidArgument);
    }

    #[allow(unused_variables)]
    fn process_foreign_instruction(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        instruction_data: &[u8],
    ) -> ProgramResult {
        Err(ProgramError::InvalidInstructionData)
    }
}
