use std::ops::{Deref, DerefMut};

use anyhow::{anyhow, bail, ensure, Context, Error, Result as PoolResult};
use arrayref::{array_refs, mut_array_refs};
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

use pool_schema::{PoolRequest, PoolRequestInner, PoolState};

struct ProgramContext<'a, 'b: 'a> {
    program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'b>],
    instruction_data: &'a [u8],
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
        Err(e) => {
            let s = format!("error processing instructions: {:?}", e);
            info!(&s);
            Err(ProgramError::Custom(0x100))
        }
    }
}

impl<'a, 'b: 'a> ProgramContext<'a, 'b> {
    fn get_account(&self, address: Pubkey) -> PoolResult<&'a AccountInfo<'b>> {
        self.accounts
            .binary_search_by_key(&address, |account_info| *account_info.key)
            .map_err(|_| anyhow!("account not found: {:?}", address))
            .map(|i| &self.accounts[i])
    }

    fn get_request(&self) -> PoolResult<PoolRequest> {
        BorshDeserialize::try_from_slice(self.instruction_data)
            .map_err(Error::msg)
            .context("failed to deserialize pool request")
    }

    fn get_state(&self, account: &'a AccountInfo<'b>) -> PoolResult<Option<PoolState>> {
        ensure!(
            account.owner == self.program_id,
            "state account isn't owned by the pool program"
        );
        BorshDeserialize::try_from_slice(account.try_borrow_data().map_err(Error::msg)?.deref())
            .map_err(Error::msg)
    }

    fn set_state(&self, account: &'a AccountInfo<'b>, state: PoolState) -> PoolResult<()> {
        let mut buf = account.try_borrow_mut_data().map_err(Error::msg)?;
        state.serialize(buf.deref_mut()).map_err(Error::msg)
    }

    fn process_instruction(&self) -> PoolResult<()> {
        let request = self.get_request()?;
        let pool_account = self
            .get_account(request.state.into())
            .context("failed to look up pool account")?;
        let pool_state = self.get_state(pool_account)?;

        match (pool_state, request.inner) {
            (None, PoolRequestInner::InitPool(state)) => {
                self.set_state(pool_account, state)
                    .context("failed to initialize pool")?;
            }
            (None, _) => bail!("uninitialized pool"),
            (Some(pool_state), PoolRequestInner::InitPool(_)) => bail!("pool already initialized"),
            (Some(pool_state), PoolRequestInner::RefreshBasket) => {}
            (Some(pool_state), PoolRequestInner::Creation(_)) => bail!("todo"),
            (Some(pool_state), PoolRequestInner::Redemption(_)) => bail!("todo"),
            (
                Some(pool_state),
                PoolRequestInner::Admin {
                    admin_signature,
                    admin_request,
                },
            ) => {
                bail!("todo");
            }
        };

        Ok(())
    }
}
