use std::ops::{Deref, DerefMut};

//use anyhow::{anyhow, bail, ensure, Context, Error, Result as PoolResult};
//use thiserror::Error;
use arrayref::{array_refs, mut_array_refs, array_ref};
use borsh::{BorshDeserialize, BorshSerialize};
pub use solana_sdk;
use solana_sdk::{
    account_info::AccountInfo,
    entrypoint,
    entrypoint::ProgramResult,
    info,
    instruction::{AccountMeta, Instruction},
    program::invoke,
    program_error::ProgramError,
    pubkey::Pubkey,
};

use serum_pool_schema::{PoolRequest, PoolRequestInner, PoolState, PoolRequestTag};

pub mod pool;

#[macro_export]
macro_rules! declare_pool_entrypoint {
    ($PoolImpl:ty) => {
        fn entrypoint(
            program_id: & $crate::solana_sdk::pubkey::Pubkey,
            accounts: &[$crate::solana_sdk::account_info::AccountInfo],
            instruction_data: &[u8],
        ) -> ProgramResult {
            $crate::pool_entrypoint::<$PoolImpl>(program_id, accounts, instruction_data)
        }
    }
}

#[inline(always)]
pub fn pool_entrypoint<P: pool::Pool>(program_id: &Pubkey, accounts: &[AccountInfo], instruction_data: &[u8]) -> ProgramResult {
    if instruction_data.len() >= 8 {
        let tag_bytes = array_ref![instruction_data, 0, 8];
        if u64::from_le_bytes(*tag_bytes) == PoolRequestTag::TAG_VALUE {
            let request = BorshDeserialize::try_from_slice(instruction_data).or(Err(ProgramError::InvalidInstructionData))?;
            return P::process_pool_request(program_id, accounts, &request);
        }
    }
    P::process_other_instruction(program_id, accounts, instruction_data)
}

/*
EXAMPLE. TODO replace with actual documentation

enum FakePool {}

impl pool::Pool for FakePool {
    fn process_other_instruction(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        instruction_data: &[u8],
    ) -> ProgramResult {
        unimplemented!()
    }
    fn process_pool_request(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        request: &PoolRequest,
    ) -> ProgramResult {
        unimplemented!()
    }
}

#[cfg(feature = "program")]
declare_pool_entrypoint!(FakePool);
*/