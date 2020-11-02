//! Program entrypoint.

#![cfg_attr(feature = "strict", deny(warnings))]

use serum_pool::context::PoolContext;
use serum_pool::pool::Pool;
use serum_pool_schema::{Basket, PoolState};
use solana_sdk::program_error::ProgramError;

mod creation;
mod get_basket;
mod initialize_pool;
mod redemption;

struct StakeProgram;

impl Pool for StakeProgram {
    fn initialize_pool(ctx: &PoolContext, state: &mut PoolState) -> Result<(), ProgramError> {
        initialize_pool::handler(ctx, state).map_err(Into::into)
    }

    fn process_creation(
        ctx: &PoolContext,
        state: &mut PoolState,
        spt_amount: u64,
    ) -> Result<(), ProgramError> {
        creation::handler(ctx, state, spt_amount).map_err(Into::into)
    }

    fn process_redemption(
        ctx: &PoolContext,
        state: &mut PoolState,
        spt_amount: u64,
    ) -> Result<(), ProgramError> {
        redemption::handler(ctx, state, spt_amount).map_err(Into::into)
    }

    fn get_creation_basket(
        ctx: &PoolContext,
        state: &PoolState,
        spt_amount: u64,
    ) -> Result<Basket, ProgramError> {
        get_basket::handler(ctx, state, spt_amount, true).map_err(Into::into)
    }

    fn get_redemption_basket(
        ctx: &PoolContext,
        state: &PoolState,
        spt_amount: u64,
    ) -> Result<Basket, ProgramError> {
        get_basket::handler(ctx, state, spt_amount, false).map_err(Into::into)
    }
}

serum_pool::declare_pool_entrypoint!(StakeProgram);
