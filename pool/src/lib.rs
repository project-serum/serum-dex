pub use solana_sdk;

pub mod schema;

use std::ops::DerefMut;

use arrayref::{array_ref, array_refs};

use solana_sdk::{
    info,
    account_info::AccountInfo,
    instruction::{Instruction, AccountMeta},
    entrypoint::ProgramResult,
    program_error::ProgramError,
    entrypoint_deprecated,
    pubkey::Pubkey,
    program::invoke,
};

use schema::{
    pool_capnp::{
        proxy_request,
    },
    cpi_capnp::{
        cpi_instr,
    },
};

use capnp::{
    message,
    traits::{Owned, HasTypeId},
    serialize::{read_message_from_flat_slice, write_message_to_words, SliceSegments},
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
    #[error("byte cast error: {0}")]
    PodCastError(bytemuck::PodCastError),
    #[error("{}", .0)]
    Msg(&'static str),
    #[error("unimplemented")]
    Todo,
}

impl From<&'static str> for PoolError {
    #[inline(always)]
    fn from(value: &'static str) -> PoolError {
        PoolError::Msg(value)
    }
}

impl From<bytemuck::PodCastError> for PoolError {
    #[inline(always)]
    fn from(value: bytemuck::PodCastError) -> PoolError {
        PoolError::PodCastError(value)
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
    Ok(tagged.get_inner_instruction()?)
}

#[cfg(feature = "program")]
entrypoint_deprecated!(entry);
fn entry(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    // TODO map errors into solana error codes or log them or something
    match process_instruction(program_id, accounts, instruction_data) {
        Ok(()) => Ok(()),
        Err(e) => match e {
            PoolError::Program(e) => Err(e),
            e => {
                let s = format!("error processing instructions: {:?}", e);
                info!(&s);
                Err(ProgramError::Custom(0x100))
            }
        }
    }
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
    let reader: proxy_request::Reader<'_> = strip_instruction_tag(cpi_instr_reader)?;

    let retbuf_account = &accounts.get(reader.get_retbuf_account_ref() as usize).ok_or("bad retbuf account ref")?;
    let retbuf_program = &accounts.get(reader.get_retbuf_program_id_ref() as usize).ok_or("bad retbuf program id ref")?;

    let required_params = {
        let range = reader.get_required_params_range();
        let begin = range.get_begin_ref() as usize;
        let end = begin + range.get_count() as usize;
        &accounts.get(begin..end).ok_or("bad required params range")?;
    };

    use proxy_request::instruction::Which;
    match reader.get_instruction().which().or(Err("unrecognized pool proxy instruction type"))? {
        Which::RefreshBasket(()) => Err(PoolError::Todo),
        Which::CreateShares(_reader) => Err(PoolError::Todo),
        Which::RedeemShares(_reader) => Err(PoolError::Todo),
        Which::AcceptAdmin(_reader) => Err(PoolError::Todo),
        Which::AdminRequest(_reader) => Err(PoolError::Todo),
    }
}

fn process_accept_admin(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> PoolResult<()> {
    unimplemented!()
}
