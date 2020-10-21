use solana_sdk::program_error::ProgramError;
use solana_sdk::{account_info::AccountInfo, entrypoint::ProgramResult, pubkey::Pubkey};

use serum_pool_schema::{Basket, PoolState};

use crate::context::PoolContext;

pub trait Pool {
    fn initialize_pool(context: &PoolContext, state: &mut PoolState) -> Result<(), ProgramError> {
        let _ = context;
        let _ = state;
        Ok(())
    }

    fn get_creation_basket(
        context: &PoolContext,
        state: &PoolState,
        request: u64,
    ) -> Result<Basket, ProgramError> {
        let _ = state;
        Ok(context.get_simple_basket(request)?)
    }

    fn get_redemption_basket(
        context: &PoolContext,
        state: &PoolState,
        request: u64,
    ) -> Result<Basket, ProgramError> {
        let _ = state;
        context.get_simple_basket(request)
    }

    #[allow(unused_variables)]
    fn process_creation(
        context: &PoolContext,
        state: &mut PoolState,
        request: u64,
    ) -> Result<(), ProgramError> {
        // TODO
        unimplemented!()
    }

    #[allow(unused_variables)]
    fn process_redemption(
        context: &PoolContext,
        state: &mut PoolState,
        request: u64,
    ) -> Result<(), ProgramError> {
        // TODO
        unimplemented!()
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
