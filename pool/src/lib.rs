pub use solana_sdk;

pub mod schema;

use arrayref::array_ref;

use solana_sdk::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    program_error::ProgramError,
    entrypoint_deprecated,
    pubkey::Pubkey,
};

use schema::{
    pool_proxy_capnp::{
        proxy_request,
    },
    pool_capnp::{
        pool_request,
    },
    cpi_capnp::{
        cpi_instr,
    },
};

use capnp::{
    NotInSchema,
    message::{self, ReaderSegments, TypedReader},
    traits::{Owned, HasTypeId},
    serialize::{read_message_from_flat_slice, SliceSegments},
};

use thiserror::Error;


#[derive(Error, Debug)]
enum PoolError {
    #[error("capnp error: {0:?}")]
    Capnp(#[from] capnp::Error),
    #[error(transparent)]
    NotInSchema(#[from] capnp::NotInSchema),
    #[error("program error: {0:?}")]
    Program(#[from] ProgramError),
    #[error("{}", .0)]
    Msg(&'static str),
}

impl From<&'static str> for PoolError {
    #[inline(always)]
    fn from(value: &'static str) -> PoolError {
        PoolError::Msg(value)
    }
}

type PoolResult<T> = Result<T, PoolError>;

fn strip_instruction_tag<'a, T>(
    tagged: cpi_instr::Reader<'a, T>,
) -> PoolResult<<T as Owned<'a>>::Reader>
where
    T: for<'b> Owned<'b>,
    <T as Owned<'a>>::Reader: HasTypeId,
{
    let instr_type_id = tagged.reborrow().get_type_id();
    if instr_type_id != T::Reader::type_id() {
        return Err("type_id not recognized".into())
    }
    Ok(tagged.get_message()?)
}

#[cfg(feature = "program")]
entrypoint_deprecated!(entry);
fn entry(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    Ok(process_instruction(program_id, accounts, instruction_data).unwrap())
}
fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> PoolResult<()> {
    let mut instruction_data_cursor = instruction_data;
    let msg_reader: message::Reader<SliceSegments<'_>> = read_message_from_flat_slice(
        &mut instruction_data_cursor,
        capnp::message::DEFAULT_READER_OPTIONS,
    )?;
    let cpi_instr_reader: cpi_instr::Reader<'_, proxy_request::Owned> = msg_reader.get_root()?;
    let proxy_request_reader: proxy_request::Reader<'_> = strip_instruction_tag(cpi_instr_reader)?;
    use proxy_request::Which::*;
    match (proxy_request_reader.which()?, accounts.len()) {
        (InitProxy(()), 2) => {
            let &[ref proxy, ref pool] = array_ref![accounts, 0, 2];
            init_proxy(program_id, proxy, pool)?;
        }
        _ => unimplemented!()
    };
    unimplemented!()
}

fn proxy_pool_request(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    request: pool_request::Reader,
) -> PoolResult<()> {
    Ok(())
}

fn init_proxy(
    program_id: &Pubkey,
    proxy: &AccountInfo,
    pool: &AccountInfo,
) -> PoolResult<()> {
    Ok(())
}
