#![allow(dead_code)]

use std::marker::PhantomData;
use std::ops::DerefMut;

use arrayref::array_ref;
use borsh::{BorshDeserialize, BorshSerialize};
pub use solana_sdk;
use solana_sdk::program_pack::Pack;
use solana_sdk::{
    account_info::AccountInfo, entrypoint::ProgramResult, program_error::ProgramError,
    pubkey::Pubkey,
    info,
};
use spl_token::state::Account as TokenAccount;

use serum_pool_schema::{AssetInfo, InitializePoolRequest, PoolAction, PoolRequest, PoolState};
use serum_pool_schema::{PoolRequestInner, PoolRequestTag};

use crate::context::PoolContext;
use crate::pool::Pool;

pub mod context;
pub mod pool;

type PoolResult<T> = Result<T, ProgramError>;

#[macro_export]
macro_rules! declare_pool_entrypoint {
    ($PoolImpl:ty) => {
        solana_sdk::entrypoint!(entry);
        fn entry(
            program_id: &$crate::solana_sdk::pubkey::Pubkey,
            accounts: &[$crate::solana_sdk::account_info::AccountInfo],
            instruction_data: &[u8],
        ) -> ProgramResult {
            $crate::pool_entrypoint::<$PoolImpl>(program_id, accounts, instruction_data)
        }
    };
}

#[inline(always)]
pub fn pool_entrypoint<P: pool::Pool>(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    if instruction_data.len() >= 8 {
        let tag_bytes = array_ref![instruction_data, 0, 8];
        if u64::from_le_bytes(*tag_bytes) == PoolRequestTag::TAG_VALUE {
            let request: PoolRequest = BorshDeserialize::try_from_slice(instruction_data)
                .map_err(|_| ProgramError::InvalidInstructionData)?;
            return PoolProcessor::<'_, '_, P> {
                program_id,
                accounts,
                request: request.inner,
                pool: PhantomData,
            }
            .process_instruction();
        }
    }
    P::process_foreign_instruction(program_id, accounts, instruction_data)
}

struct PoolProcessor<'a, 'b, P> {
    program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'b>],
    request: PoolRequestInner,
    pool: std::marker::PhantomData<P>,
}

impl<'a, 'b, P: Pool> PoolProcessor<'a, 'b, P> {
    #[inline(never)]
    fn get_state(&self) -> PoolResult<Option<PoolState>> {
        if self.accounts.len() < 1 {
            return Err(ProgramError::NotEnoughAccountKeys);
        }
        let account = &self.accounts[0];
        if account.owner != self.program_id {
            return Err(ProgramError::IncorrectProgramId);
        };
        let data = account.try_borrow_data()?;
        if data.iter().all(|b| *b == 0) {
            return Ok(None);
        }
        // Can't use BorshDeserialize::try_from_slice because try_from_slice expects the data to
        // take up the entire slice.
        let mut data: &[u8] = *data;
        Ok(Some(
            BorshDeserialize::deserialize(&mut data)
                .map_err(|_| ProgramError::InvalidAccountData)?,
        ))
    }

    fn set_state(&self, state: PoolState) -> PoolResult<()> {
        if self.accounts.len() < 1 {
            return Err(ProgramError::NotEnoughAccountKeys);
        }
        let account = &self.accounts[0];
        let mut buf = account.try_borrow_mut_data()?;
        BorshSerialize::serialize(&state, buf.deref_mut())
            .map_err(|_| ProgramError::AccountDataTooSmall)
    }

    fn process_instruction(&self) -> PoolResult<()> {
        let mut pool_state = self.get_state()?;

        match (&mut pool_state, &self.request) {
            (None, PoolRequestInner::Initialize(request)) => self.initialize_pool(request)?,
            (None, _) => {
                return Err(ProgramError::UninitializedAccount);
            }
            (Some(_), PoolRequestInner::Initialize(_)) => {
                return Err(ProgramError::AccountAlreadyInitialized);
            }
            (Some(pool_state), PoolRequestInner::GetBasket(action)) => {
                let context =
                    PoolContext::new(self.program_id, self.accounts, &pool_state, &self.request)?;
                match action {
                    &PoolAction::Create(amount) => {
                        P::get_creation_basket(&context, pool_state, amount)?
                    }
                    &PoolAction::Redeem(amount) => {
                        P::get_redemption_basket(&context, pool_state, amount)?
                    }
                    PoolAction::Swap(inputs) => P::get_swap_basket(&context, pool_state, inputs)?,
                };
            }
            (Some(pool_state), PoolRequestInner::Transact(action)) => {
                let context =
                    PoolContext::new(self.program_id, self.accounts, &pool_state, &self.request)?;
                match action {
                    &PoolAction::Create(amount) => {
                        P::process_creation(&context, pool_state, amount)?
                    }
                    &PoolAction::Redeem(amount) => {
                        P::process_redemption(&context, pool_state, amount)?
                    }
                    PoolAction::Swap(inputs) => P::process_swap(&context, pool_state, inputs)?,
                };
            }
        };

        Ok(())
    }

    fn initialize_pool(&self, request: &InitializePoolRequest) -> PoolResult<()> {
        let mut state = PoolState {
            tag: Default::default(),
            pool_token_mint: self.accounts[1].key.into(),
            assets: self.accounts[2..2 + request.assets_length as usize]
                .iter()
                .map(|account| {
                    let acc = TokenAccount::unpack(&account.try_borrow_data()?)?;
                    Ok(AssetInfo {
                        mint: acc.mint.into(),
                        vault_address: account.key.into(),
                    })
                })
                .collect::<PoolResult<Vec<_>>>()?,
            vault_signer: self.accounts[2 + request.assets_length as usize].key.into(),
            vault_signer_nonce: request.vault_signer_nonce,
            account_params: vec![],
            admin_key: None,
            custom_state: vec![],
        };
        let context = PoolContext::new(self.program_id, self.accounts, &state, &self.request)?;

        context.check_rent_exemption(context.pool_account)?;
        context.check_rent_exemption(context.pool_token_mint)?;
        for vault_account in context.pool_vault_accounts {
            context.check_rent_exemption(vault_account)?;
        }

        P::initialize_pool(&context, &mut state)?;
        if *context.pool_authority.key != context.derive_vault_authority(&state)? {
            info!("Invalid pool authority");
            return Err(ProgramError::InvalidArgument)
        }
        self.set_state(state)?;
        Ok(())
    }
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
