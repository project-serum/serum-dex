use solana_sdk::program_error::ProgramError;
use solana_sdk::{account_info::AccountInfo, entrypoint::ProgramResult, pubkey::Pubkey};

use serum_pool_schema::{Basket, PoolState};

use crate::context::PoolContext;

pub trait Pool {
    #[allow(unused_variables)]
    fn initialize_pool(context: &PoolContext, state: &mut PoolState) -> Result<(), ProgramError> {
        unimplemented!()
    }

    #[allow(unused_variables)]
    fn get_creation_basket(
        context: &PoolContext,
        state: &PoolState,
        request: u64,
    ) -> Result<Basket, ProgramError> {
        context.get_simple_basket(request)
    }

    #[allow(unused_variables)]
    fn get_redemption_basket(
        context: &PoolContext,
        state: &PoolState,
        request: u64,
    ) -> Result<Basket, ProgramError> {
        context.get_simple_basket(request)
    }

    #[allow(unused_variables)]
    fn get_swap_basket(
        context: &PoolContext,
        state: &PoolState,
        request: &[u64],
    ) -> Result<Basket, ProgramError> {
        return Err(ProgramError::InvalidArgument)
    }

    #[allow(unused_variables)]
    fn process_creation(
        context: &PoolContext,
        state: &mut PoolState,
        request: u64,
    ) -> Result<(), ProgramError> {
        let basket = Self::get_creation_basket(context, state, request)?;
        unimplemented!()
    }

    #[allow(unused_variables)]
    fn process_redemption(
        context: &PoolContext,
        state: &mut PoolState,
        request: u64,
    ) -> Result<(), ProgramError> {
        unimplemented!()
    }

    #[allow(unused_variables)]
    fn process_swap(
        context: &PoolContext,
        state: &mut PoolState,
        request: &[u64],
    ) -> Result<(), ProgramError> {
        return Err(ProgramError::InvalidArgument)
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
