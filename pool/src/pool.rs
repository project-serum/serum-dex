use std::convert::TryInto;

use solana_sdk::{
    account_info::AccountInfo, entrypoint::ProgramResult, program, program_error::ProgramError,
    pubkey::Pubkey,
};

use serum_pool_schema::{Basket, PoolState};

use crate::context::PoolContext;

pub trait Pool {
    #[allow(unused_variables)]
    fn initialize_pool(context: &PoolContext, state: &mut PoolState) -> Result<(), ProgramError> {
        Ok(())
    }

    #[allow(unused_variables)]
    fn get_creation_basket(
        context: &PoolContext,
        state: &PoolState,
        request: u64,
    ) -> Result<Basket, ProgramError> {
        context.get_simple_basket(request, true)
    }

    #[allow(unused_variables)]
    fn get_redemption_basket(
        context: &PoolContext,
        state: &PoolState,
        request: u64,
    ) -> Result<Basket, ProgramError> {
        context.get_simple_basket(request, false)
    }

    #[allow(unused_variables)]
    fn get_swap_basket(
        context: &PoolContext,
        state: &PoolState,
        request: &[u64],
    ) -> Result<Basket, ProgramError> {
        return Err(ProgramError::InvalidArgument);
    }

    #[allow(unused_variables)]
    fn process_creation(
        context: &PoolContext,
        state: &mut PoolState,
        request: u64,
    ) -> Result<(), ProgramError> {
        let basket = Self::get_creation_basket(context, state, request)?;
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
        {
            let mint_pubkey = context.pool_token_mint.key;
            let account_pubkey = user_accounts.pool_token_account.key;
            let owner_pubkey = context.pool_authority.key;
            let signer_pubkeys = &[];

            let instruction = spl_token::instruction::mint_to(
                &spl_token::ID,
                mint_pubkey,
                account_pubkey,
                owner_pubkey,
                signer_pubkeys,
                request,
            )?;

            let account_infos = &[
                user_accounts.pool_token_account.clone(),
                context.pool_token_mint.clone(),
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
        };
        Ok(())
    }

    #[allow(unused_variables)]
    fn process_redemption(
        context: &PoolContext,
        state: &mut PoolState,
        request: u64,
    ) -> Result<(), ProgramError> {
        let basket = Self::get_redemption_basket(context, state, request)?;
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

        // pull in shares
        {
            let mint_pubkey = context.pool_token_mint.key;
            let account_pubkey = user_accounts.pool_token_account.key;
            let authority_pubkey = user_accounts.authority.key;
            let signer_pubkeys = &[];

            let instruction = spl_token::instruction::burn(
                &spl_token::ID,
                account_pubkey,
                mint_pubkey,
                authority_pubkey,
                signer_pubkeys,
                request,
            )?;

            let account_infos = &[
                context.pool_token_mint.clone(),
                user_accounts.pool_token_account.clone(),
                user_accounts.authority.clone(),
                spl_token_program.clone(),
            ];

            program::invoke(&instruction, account_infos)?;
        }

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

    fn process_foreign_instruction(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        instruction_data: &[u8],
    ) -> ProgramResult {
        let _ = program_id;
        let _ = accounts;
        let _ = instruction_data;
        Err(ProgramError::InvalidInstructionData)
    }
}
