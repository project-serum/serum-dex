pub use solana_sdk;

pub mod schema;

use std::ops::{Deref, DerefMut};

use solana_sdk::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    entrypoint_deprecated, info,
    instruction::{AccountMeta, Instruction},
    program::invoke,
    program_error::ProgramError,
    pubkey::Pubkey,
};

use schema::{cpi_capnp::{self, cpi_instr}, pool_capnp::{proxy_request, proxy_account, basket}};

use capnp::{
    message,
    serialize::{read_message, write_message},
    traits::{HasTypeId, Owned},
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
    #[error("missing account info")]
    MissingAccount([u8; 32]), // TODO include address in error message
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

struct ProgramContext<'a, 'b: 'a> {
    program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'b>],
    instruction_data: &'a [u8],
}

#[inline]
fn deserialize_address(reader: cpi_capnp::address::Reader) -> Pubkey {
    use slice_of_array::prelude::*;
    Pubkey::new([
        reader.get_word0().to_le_bytes(),
        reader.get_word1().to_le_bytes(),
        reader.get_word2().to_le_bytes(),
        reader.get_word3().to_le_bytes(),
    ].flat())
}

fn strip_instruction_tag<'a, T>(
    tagged: cpi_instr::Reader<'a, T>,
) -> PoolResult<<T as Owned<'a>>::Reader>
where
    T: for<'b> Owned<'b>,
    <T as Owned<'a>>::Reader: HasTypeId,
{
    let instr_type_id = tagged.reborrow().get_type_id();
    if instr_type_id != T::Reader::type_id() {
        return Err("type_id not recognized".into());
    }
    Ok(tagged.get_inner_instruction()?)
}

#[cfg(feature = "program")]
entrypoint_deprecated!(entry);
fn entry(program_id: &Pubkey, accounts: &[AccountInfo], instruction_data: &[u8]) -> ProgramResult {
    let context = ProgramContext {
        program_id,
        accounts,
        instruction_data,
    };

    match context.process_instruction() {
        Ok(()) => Ok(()),
        Err(PoolError::Program(e)) => Err(e),
        Err(e) => {
            let s = format!("error processing instructions: {:?}", e);
            info!(&s);
            Err(ProgramError::Custom(0x100))
        }
    }
}

impl<'a, 'b: 'a> ProgramContext<'a, 'b> {
    fn lookup_account_info(&self, reader: cpi_capnp::account_info::Reader)
        -> PoolResult<&'a AccountInfo<'b>>
    {
        let desired_address = deserialize_address(reader.get_address()?);
        self.accounts.binary_search_by_key(
            &desired_address,
            |account_info| *account_info.key
        ).map_err(|_| PoolError::MissingAccount(desired_address.to_bytes()))
        .map(|i| &self.accounts[i])
    }

    fn process_instruction(&self) -> PoolResult<()> {
        let mut instruction_data_cursor = self.instruction_data;
        let msg_reader = read_message(
            &mut instruction_data_cursor,
            capnp::message::DEFAULT_READER_OPTIONS,
        )?;
        let cpi_instr_reader: cpi_instr::Reader<proxy_request::Owned> = msg_reader.get_root()?;
        let reader: proxy_request::Reader<'_> = strip_instruction_tag(cpi_instr_reader)?;

        let root_msg;
        let root_state_account;
        let state_reader: proxy_account::Reader = {
            root_state_account = self.lookup_account_info(reader.get_state_root()?)?;
            if root_state_account.owner != self.program_id {
                return Err("wrong account owner".into());
            }
            let root_state_ref = root_state_account.try_borrow_data()?;
            let mut root_state_cursor: &[u8] = root_state_ref.deref();
            root_msg = read_message(
                &mut root_state_cursor,
                capnp::message::DEFAULT_READER_OPTIONS,
            )?;
            root_msg.get_root()?
        };
        let state = state_reader.which().or(Err("invalid account state tag"))?;

        let instruction = reader
            .get_instruction()
            .which()
            .or(Err("invalid proxy instruction tag"))?;
        use proxy_request::instruction::Which as InstrTag;
        use proxy_account::Which as StateTag;
        match (instruction, state) {
            (InstrTag::RefreshBasket(()), StateTag::ProxyState(Ok(state))) => {
                let basket = state.get_basket()?;
                match basket.which().or(Err("invalid basket tag"))? {
                    basket::Static(_) => Err("can't refresh a static basket".into()),
                    basket::Dynamic(_) => Err(PoolError::Todo),
                }
            }

            (InstrTag::CreateOrRedeem(_instr), StateTag::ProxyState(Ok(_state))) => Err(PoolError::Todo),

            (InstrTag::AcceptAdmin(instr), StateTag::ProxyState(Ok(state))) => {
                let acceptor = self
                    .lookup_account_info(
                        instr.get_pending_admin_signature()?
                    )?;
                let pending_admin_reader = state.get_pending_admin_key()?;
                let pending_admin = deserialize_address(pending_admin_reader);
                if acceptor.signer_key() != Some(&pending_admin) {
                    return Err("not authorized to accept admin".into());
                }
                let mut msg_builder = message::Builder::new_default();
                {
                    let proxy_account_builder: proxy_account::Builder = msg_builder.init_root();
                    let mut proxy_state_builder = proxy_account_builder.init_proxy_state();

                    proxy_state_builder.set_basket(state.get_basket()?)?;
                    proxy_state_builder.set_admin_key(pending_admin_reader)?;
                    proxy_state_builder.set_pool_token(state.get_pool_token()?)?;
                }
                let mut root_state_ref_mut = root_state_account.try_borrow_mut_data()?;
                let mut root_state_cursor = root_state_ref_mut.deref_mut();
                Ok(write_message(&mut root_state_cursor, &msg_builder)?)
            }

            (InstrTag::AdminRequest(instr), StateTag::ProxyState(Ok(state))) => {
                use proxy_request::instruction::admin_request;
                {
                    let signer = self
                        .lookup_account_info(
                            instr.get_admin_signature()?
                        )?;
                    let admin = deserialize_address(state.get_admin_key()?);
                    if signer.signer_key() != Some(&admin) {
                        return Err("invalid admin signature".into());
                    }
                }

                let mut msg_builder = message::Builder::new_default();
                {
                    let proxy_account_builder: proxy_account::Builder = msg_builder.init_root();
                    let mut proxy_state_builder = proxy_account_builder.init_proxy_state();

                    proxy_state_builder.set_pool_token(state.get_pool_token()?)?;
                    proxy_state_builder.set_admin_key(state.get_admin_key()?)?;
                    match instr.which().or(Err("invalid admin request"))? {
                        admin_request::SetPendingAdmin(pending_admin) => {
                            proxy_state_builder.set_pending_admin_key(pending_admin?)?;
                            proxy_state_builder.set_basket(state.get_basket()?)?;
                        }
                        admin_request::SetBasket(new_basket) => {
                            proxy_state_builder.set_pending_admin_key(state.get_pending_admin_key()?)?;
                            proxy_state_builder.set_basket(new_basket?)?;
                        },
                    };
                }
                let mut root_state_ref_mut = root_state_account.try_borrow_mut_data()?;
                let mut root_state_cursor = root_state_ref_mut.deref_mut();
                Ok(write_message(&mut root_state_cursor, &msg_builder)?)
            }

            (InstrTag::InitProxy(init_proxy_reader), StateTag::Unset(())) => {
                let mut msg_builder = message::Builder::new_default();
                {
                    let proxy_account_builder: proxy_account::Builder = msg_builder.init_root();
                    let mut proxy_state_builder = proxy_account_builder.init_proxy_state();

                    proxy_state_builder.set_basket(init_proxy_reader.get_basket()?)?;
                    proxy_state_builder.set_admin_key(init_proxy_reader.get_admin_key()?)?;

                    // TODO validate the pool token
                    proxy_state_builder.set_pool_token(init_proxy_reader.get_pool_token()?)?;
                }
                let mut root_state_ref_mut = root_state_account.try_borrow_mut_data()?;
                let mut root_state_cursor = root_state_ref_mut.deref_mut();
                Ok(write_message(&mut root_state_cursor, &msg_builder)?)
            }   

            (_, StateTag::ProxyState(Err(e))) => return Err(e.into()),
            (_, StateTag::ProxyState(_)) => Err("account already initialized".into()),
            (_, StateTag::Unset(_)) => Err("account not initialized".into()),
        }
    }
}
