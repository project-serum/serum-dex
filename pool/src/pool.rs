use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, program_error::ProgramError,
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
        context.transfer_basket_from_user(&basket)?;
        context.mint_tokens(state, creation_size)?;
        Ok(())
    }

    fn process_redemption(
        context: &PoolContext,
        state: &mut PoolState,
        redemption_size: u64,
    ) -> Result<(), ProgramError> {
        let fees = context.get_fees(state, redemption_size)?;
        let redemption_size = redemption_size - fees.total_fee();
        let basket = Self::get_redemption_basket(context, state, redemption_size)?;
        context.burn_tokens_and_collect_fees(redemption_size, fees)?;
        context.transfer_basket_to_user(state, &basket)?;
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
