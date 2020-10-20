#![allow(dead_code)]

use std::ops::DerefMut;

use anyhow::{bail, ensure, Context, Error, Result as PoolResult};
use borsh::{BorshDeserialize, BorshSerialize};
pub use solana_sdk;
use solana_sdk::{
    account_info::AccountInfo, entrypoint, entrypoint::ProgramResult, info,
    program_error::ProgramError, pubkey::Pubkey,
};

use serum_pool_schema::{AssetInfo, Basket, PoolRequest, PoolState};

use crate::context::PoolContext;

mod context;

struct ProgramContext<'a, 'b: 'a> {
    program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'b>],
    instruction_data: &'a [u8],
}

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
}

struct Foo {}
impl Pool for Foo {}

#[cfg(feature = "program")]
entrypoint!(entry);
fn entry(program_id: &Pubkey, accounts: &[AccountInfo], instruction_data: &[u8]) -> ProgramResult {
    let context = ProgramContext {
        program_id,
        accounts,
        instruction_data,
    };

    match context.process_instruction::<Foo>() {
        Ok(()) => Ok(()),
        Err(e) => {
            let s = format!("error processing instructions: {:?}", e);
            info!(&s);
            Err(ProgramError::Custom(0x100))
        }
    }
}

impl<'a, 'b: 'a> ProgramContext<'a, 'b> {
    fn get_request(&self) -> PoolResult<PoolRequest> {
        BorshDeserialize::try_from_slice(self.instruction_data)
            .map_err(Error::msg)
            .context("failed to deserialize pool request")
    }

    #[inline(never)]
    fn get_state(&self, account: &AccountInfo) -> PoolResult<Option<PoolState>> {
        ensure!(
            account.owner == self.program_id,
            "state account isn't owned by the pool program"
        );
        let data = account.try_borrow_data().map_err(Error::msg)?;
        if data.iter().all(|b| *b == 0) {
            return Ok(None);
        }
        // Can't use BorshDeserialize::try_from_slice because try_from_slice expects the data to
        // take up the entire slice.
        let mut data: &[u8] = *data;
        Ok(Some(
            BorshDeserialize::deserialize(&mut data)
                .map_err(Error::msg)
                .context("failed to deserialize state")?,
        ))
    }

    fn set_state(&self, account: &AccountInfo, state: PoolState) -> PoolResult<()> {
        let mut buf = account.try_borrow_mut_data().map_err(Error::msg)?;
        BorshSerialize::serialize(&state, buf.deref_mut())
            .map_err(Error::msg)
            .context("failed to serialize state")
    }

    fn process_instruction<P: Pool>(&self) -> PoolResult<()> {
        let request = self.get_request()?;
        let mut pool_state = self.get_state(&self.accounts[0])?;

        match (&mut pool_state, &request) {
            (None, PoolRequest::Initialize(init_request)) => {
                let mut state = PoolState {
                    initialized: true,
                    pool_token_mint: self.accounts[1].key.into(),
                    assets: self.accounts[2..2 + init_request.assets_length as usize]
                        .iter()
                        .map(|account| AssetInfo {
                            mint: account.key.into(), // TODO
                            vault_address: account.key.into(),
                        })
                        .collect(),
                    vault_signer: self.accounts[2 + init_request.assets_length as usize]
                        .key
                        .into(),
                    vault_signer_nonce: init_request.vault_signer_nonce,
                    account_params: vec![],
                    admin_key: None,
                    custom_state: vec![],
                };
                // TODO: validate state
                let context = PoolContext::new(self.program_id, self.accounts, &state, &request)
                    .map_err(Error::msg)?;
                P::initialize_pool(&context, &mut state).map_err(Error::msg)?;
                self.set_state(&self.accounts[0], state)?;
            }
            (None, _) => bail!("uninitialized pool"),
            (Some(_), PoolRequest::Initialize(_)) => bail!("pool already initialized"),
            (Some(_pool_state), PoolRequest::GetBasket(_request)) => {}
            (Some(_pool_state), PoolRequest::Transact(_)) => bail!("todo"),
            (Some(_pool_state), PoolRequest::AdminRequest) => bail!("todo"),
            (Some(_pool_state), PoolRequest::CustomRequest(_)) => bail!("todo"),
        };

        Ok(())
    }
}
