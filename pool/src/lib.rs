use std::ops::{Deref, DerefMut};

use arrayref::{array_refs, mut_array_refs};
use capnp::{
    message,
    serialize::{read_message, write_message},
    traits::{HasTypeId, Owned},
};
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
use thiserror::Error;

use schema::{
    cpi_capnp::{self, cpi_instr},
    pool_capnp::{basket, proxy_account, proxy_request},
};

pub mod schema;

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
    Pubkey::new(
        [
            reader.get_word0().to_le_bytes(),
            reader.get_word1().to_le_bytes(),
            reader.get_word2().to_le_bytes(),
            reader.get_word3().to_le_bytes(),
        ]
        .flat(),
    )
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
entrypoint!(entry);
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
    fn lookup_account_info(
        &self,
        reader: cpi_capnp::account_info::Reader,
    ) -> PoolResult<&'a AccountInfo<'b>> {
        let desired_address = deserialize_address(reader.get_address()?);
        self.accounts
            .binary_search_by_key(&desired_address, |account_info| *account_info.key)
            .map_err(|_| PoolError::MissingAccount(desired_address.to_bytes()))
            .map(|i| &self.accounts[i])
    }

    #[inline(never)]
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
        let state_reader: Option<proxy_account::Reader> = {
            root_state_account = self.lookup_account_info(reader.get_state_root()?)?;
            if root_state_account.owner != self.program_id {
                return Err("wrong account owner".into());
            }
            let root_state_ref = root_state_account.try_borrow_data()?;

            let (init_tag, mut root_state_cursor) = array_refs![root_state_ref.deref(), 8; ..;];
            if init_tag == &[0u8; 8] {
                None
            } else {
                root_msg = read_message(
                    &mut root_state_cursor,
                    capnp::message::DEFAULT_READER_OPTIONS,
                )?;
                Some(root_msg.get_root()?)
            }
        };
        let ref state = state_reader
            .map(|r| r.which().or(Err("invalid account state tag")))
            .transpose()?;

        let ref instruction = reader
            .get_instruction()
            .which()
            .or(Err("invalid proxy instruction tag"))?;
        use proxy_account::Which as StateTag;
        use proxy_request::instruction::Which as InstrTag;
        match state {
            &Some(StateTag::ProxyState(ref state)) => {
                let state = state.as_ref().map_err(|e| e.clone())?;
                match instruction {
                    &InstrTag::RefreshBasket(()) => {
                        let basket = state.get_basket()?;
                        match basket.which().or(Err("invalid basket tag"))? {
                            basket::Static(_) => Err("can't refresh a static basket".into()),
                            basket::Dynamic(_) => Err(PoolError::Todo),
                        }
                    }
                    &InstrTag::CreateOrRedeem(_) => Err(PoolError::Todo),
                    &InstrTag::AcceptAdmin(ref instr) => {
                        let acceptor =
                            self.lookup_account_info(instr.get_pending_admin_signature()?)?;
                        let pending_admin_reader = state.get_pending_admin_key()?;
                        let pending_admin = deserialize_address(pending_admin_reader);
                        if acceptor.signer_key() != Some(&pending_admin) {
                            return Err("not authorized to accept admin".into());
                        }
                        let mut msg_builder = message::Builder::new_default();
                        {
                            let proxy_account_builder: proxy_account::Builder =
                                msg_builder.init_root();
                            let mut proxy_state_builder = proxy_account_builder.init_proxy_state();

                            proxy_state_builder.set_basket(state.get_basket()?)?;
                            proxy_state_builder.set_admin_key(pending_admin_reader)?;
                            proxy_state_builder.set_pool_token(state.get_pool_token()?)?;
                        }
                        let mut root_state_ref_mut = root_state_account.try_borrow_mut_data()?;
                        let mut root_state_cursor = &mut root_state_ref_mut.deref_mut()[8..];

                        Ok(write_message(&mut root_state_cursor, &msg_builder)?)
                    }
                    &InstrTag::AdminRequest(ref admin_request) => {
                        let signing_admin = self
                            .lookup_account_info(admin_request.get_admin_signature()?)?
                            .signer_key();
                        let expected_admin = deserialize_address(state.get_admin_key()?);
                        if signing_admin != Some(&expected_admin) {
                            return Err("unauthorized admin signature".into());
                        }
                        use proxy_request::instruction::admin_request::Which as AdminReqTag;
                        match &admin_request.which().or(Err("invalid admin request tag"))? {
                            AdminReqTag::SetPendingAdmin(_) => Err(PoolError::Todo),
                            AdminReqTag::SetBasket(_) => Err(PoolError::Todo),
                        }
                    }
                    &InstrTag::InitProxy(_) => Err("account already initialized".into()),
                }
            }
            &None => match instruction {
                &InstrTag::InitProxy(ref init_proxy_reader) => {
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
                    let (len_prefix, tail) =
                        mut_array_refs![root_state_ref_mut.deref_mut(), 8; ..;];
                    let tail_len = tail.len();
                    let mut root_state_cursor = &mut root_state_ref_mut[..];
                    root_state_cursor[0] = 1;
                    root_state_cursor = &mut root_state_cursor[8..];
                    Ok(write_message(&mut root_state_cursor, &msg_builder)?)
                }
                _ => Err("account not initialized".into()),
            },
            &Some(StateTag::ProxyState(Err(ref e))) => {
                return Err(e.clone().into());
            }
            &Some(StateTag::Unset(())) => return Err("BUG: account incorrectly initialized".into()),
        }
    }
}
