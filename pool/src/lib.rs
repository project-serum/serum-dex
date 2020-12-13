use std::marker::PhantomData;
use std::ops::DerefMut;

use arrayref::array_ref;
use borsh::{BorshDeserialize, BorshSerialize};
pub use solana_program;
use solana_program::account_info::next_account_info;
use solana_program::program_option::COption;
use solana_program::program_pack::Pack;
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, msg, program_error::ProgramError,
    pubkey::Pubkey,
};
use spl_token::state::Account as TokenAccount;

pub use serum_pool_schema as schema;
use serum_pool_schema::{
    AssetInfo, InitializePoolRequest, PoolAction, PoolRequest, PoolRequestInner, PoolRequestTag,
    PoolState, FEE_RATE_DENOMINATOR, MIN_FEE_RATE,
};

pub use crate::context::PoolContext;
pub use crate::pool::Pool;

pub mod context;
pub mod pool;

type PoolResult<T> = Result<T, ProgramError>;

#[macro_export]
macro_rules! declare_pool_entrypoint {
    ($PoolImpl:ty) => {
        solana_program::entrypoint!(entry);
        fn entry(
            program_id: &$crate::solana_program::pubkey::Pubkey,
            accounts: &[$crate::solana_program::account_info::AccountInfo],
            instruction_data: &[u8],
        ) -> solana_program::entrypoint::ProgramResult {
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
            let request: PoolRequest =
                BorshDeserialize::try_from_slice(instruction_data).map_err(|e| {
                    msg!(&e.to_string());
                    ProgramError::InvalidInstructionData
                })?;
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
            msg!("Account not owned by pool program");
            return Err(ProgramError::IncorrectProgramId);
        };
        let data = account.try_borrow_data()?;
        if data.iter().all(|b| *b == 0) {
            return Ok(None);
        }
        // Can't use BorshDeserialize::try_from_slice because try_from_slice expects the data to
        // take up the entire slice.
        let mut data: &[u8] = *data;
        Ok(Some(BorshDeserialize::deserialize(&mut data).map_err(
            |e| {
                msg!(&e.to_string());
                ProgramError::InvalidAccountData
            },
        )?))
    }

    fn save_state(&self, state: &PoolState) -> PoolResult<()> {
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
                let basket = match action {
                    &PoolAction::Create(amount) => {
                        P::get_creation_basket(&context, pool_state, amount)?
                    }
                    &PoolAction::Redeem(amount) => {
                        P::get_redemption_basket(&context, pool_state, amount)?
                    }
                    PoolAction::Swap(inputs) => P::get_swap_basket(&context, pool_state, inputs)?,
                };
                let mut result = Vec::with_capacity(4096);
                result.extend_from_slice(&[0u8; 8]);
                basket
                    .serialize(&mut result)
                    .map_err(|_| ProgramError::InvalidInstructionData)?;
                context
                    .retbuf
                    .as_ref()
                    .ok_or(ProgramError::InvalidArgument)?
                    .write_data(result)?;
            }
            (Some(pool_state), PoolRequestInner::Execute(action)) => {
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
                self.save_state(pool_state)?;
            }
        };

        Ok(())
    }

    fn initialize_pool(&self, request: &InitializePoolRequest) -> PoolResult<()> {
        let accounts_iter = &mut self.accounts.into_iter();
        let _pool_account = next_account_info(accounts_iter)?;
        let pool_token_mint = next_account_info(accounts_iter)?;
        let pool_vaults = next_account_infos(accounts_iter, request.assets_length as usize)?;
        let vault_signer = next_account_info(accounts_iter)?;
        let lqd_fee_vault = next_account_info(accounts_iter)?;
        let initializer_fee_vault = next_account_info(accounts_iter)?;

        let mut state = PoolState {
            tag: Default::default(),
            pool_token_mint: pool_token_mint.key.into(),
            assets: pool_vaults
                .iter()
                .map(|account| {
                    let acc = TokenAccount::unpack(&account.try_borrow_data()?)?;
                    Ok(AssetInfo {
                        mint: acc.mint.into(),
                        vault_address: account.key.into(),
                    })
                })
                .collect::<PoolResult<Vec<_>>>()?,
            vault_signer: vault_signer.key.into(),
            vault_signer_nonce: request.vault_signer_nonce,
            lqd_fee_vault: lqd_fee_vault.key.into(),
            initializer_fee_vault: initializer_fee_vault.key.into(),
            fee_rate: request.fee_rate,
            account_params: vec![],
            name: request.pool_name.clone(),
            admin_key: None,
            custom_state: vec![],
        };
        let context = PoolContext::new(self.program_id, self.accounts, &state, &self.request)?;

        context.check_rent_exemption(context.pool_account)?;
        context.check_rent_exemption(context.pool_token_mint)?;
        for vault_account in context.pool_vault_accounts {
            context.check_rent_exemption(vault_account)?;
        }
        context.check_rent_exemption(lqd_fee_vault)?;
        self.check_lqd_fee_account(&state, lqd_fee_vault)?;
        context.check_rent_exemption(initializer_fee_vault)?;

        P::initialize_pool(&context, &mut state)?;
        if *context.pool_authority.key != context.derive_vault_authority(&state)? {
            msg!("Invalid pool authority");
            return Err(ProgramError::InvalidArgument);
        }
        if state.fee_rate < MIN_FEE_RATE {
            msg!("Fee too low");
            return Err(ProgramError::InvalidArgument);
        }
        if state.fee_rate >= FEE_RATE_DENOMINATOR {
            msg!("Fee too high");
            return Err(ProgramError::InvalidArgument);
        }
        self.save_state(&state)?;
        Ok(())
    }

    fn check_lqd_fee_account(
        &self,
        state: &PoolState,
        account: &AccountInfo,
    ) -> Result<(), ProgramError> {
        let token_account = TokenAccount::unpack(&account.try_borrow_data()?)?;
        if token_account.owner != serum_pool_schema::fee_owner::ID {
            msg!("Incorrect fee account owner");
            return Err(ProgramError::InvalidArgument);
        }
        if token_account.delegate.is_some() {
            msg!("Incorrect fee account delegate");
            return Err(ProgramError::InvalidArgument);
        }
        if token_account.close_authority.is_some()
            && token_account.close_authority.as_ref() != COption::Some(state.vault_signer.as_ref())
        {
            msg!("Incorrect fee account close authority");
            return Err(ProgramError::InvalidArgument);
        }
        Ok(())
    }
}

fn next_account_infos<'a, 'b: 'a>(
    iter: &mut std::slice::Iter<'a, AccountInfo<'b>>,
    count: usize,
) -> Result<&'a [AccountInfo<'b>], ProgramError> {
    let accounts = iter.as_slice();
    if accounts.len() < count {
        return Err(ProgramError::NotEnoughAccountKeys);
    }
    let (accounts, remaining) = accounts.split_at(count);
    *iter = remaining.into_iter();
    Ok(accounts)
}
