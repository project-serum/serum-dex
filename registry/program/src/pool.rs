use borsh::de::BorshDeserialize;
use serum_common::pack::Pack;
use serum_common::shared_mem;
use serum_pool_schema::{Basket, PoolAction};
use serum_registry::accounts::entity::PoolPrices;
use serum_registry::accounts::{vault, Member, Registrar};
use serum_registry::error::{RegistryError, RegistryErrorCode};
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::program_option::COption;
use solana_sdk::program_pack::Pack as TokenPack;
use solana_sdk::pubkey::Pubkey;
use spl_token::state::Account as TokenAccount;

// Pool is a CPI client for the registry to invoke the staking pool.
#[derive(Clone)]
pub struct Pool<'a, 'b> {
    is_mega: bool,
    accounts: PoolAccounts<'a, 'b>,
    mega_accounts: PoolAccounts<'a, 'b>,
    // Represents redemption prices for all instructions except `stake`,
    // in which case we use the creation prices.
    prices: PoolPrices,
}

impl<'a, 'b> std::ops::Deref for Pool<'a, 'b> {
    type Target = PoolAccounts<'a, 'b>;

    fn deref(&self) -> &Self::Target {
        if self.is_mega {
            &self.mega_accounts
        } else {
            &self.accounts
        }
    }
}

impl<'a, 'b> Pool<'a, 'b> {
    pub fn is_mega(&self) -> bool {
        self.is_mega
    }

    pub fn prices(&self) -> &PoolPrices {
        &self.prices
    }

    pub fn parse_accounts(
        acc_infos: &mut dyn std::iter::Iterator<Item = &'a AccountInfo<'b>>,
        cfg: PoolConfig<'a, 'b>,
    ) -> Result<Self, RegistryError> {
        let acc_infos = acc_infos.collect::<Vec<_>>();
        let is_mega = match acc_infos.len() {
            16 => true,
            15 => false,
            12 => false, // true/false doesn't matter since 12 => *not* PoolConfig::Execute.
            _ => return Err(RegistryErrorCode::InvalidPoolAccounts)?,
        };

        let acc_infos = &mut acc_infos.into_iter();

        // Program ids.
        let pool_program_id_acc_info = next_account_info(acc_infos)?;
        let retbuf_program_acc_info = next_account_info(acc_infos)?;
        let retbuf_acc_info = next_account_info(acc_infos)?;

        // SRM pool.
        let pool_acc_info = next_account_info(acc_infos)?;
        let pool_tok_mint_acc_info = next_account_info(acc_infos)?;
        let pool_asset_vault_acc_infos = vec![next_account_info(acc_infos)?];
        let pool_vault_authority_acc_info = next_account_info(acc_infos)?;

        // MSRM pool.
        let mega_pool_acc_info = next_account_info(acc_infos)?;
        let mega_pool_tok_mint_acc_info = next_account_info(acc_infos)?;
        let mut mega_pool_asset_vault_acc_infos = vec![next_account_info(acc_infos)?];
        mega_pool_asset_vault_acc_infos.push(next_account_info(acc_infos)?);
        let mega_pool_vault_authority_acc_info = next_account_info(acc_infos)?;

        // Transact specific params.
        let mut pool_token_acc_info = None;
        let mut registry_vault_acc_infos = None;
        let mut registry_signer_acc_info = None;
        let mut token_program_acc_info = None;
        let mut signer_seeds = None;
        let mut is_create = false;
        if let PoolConfig::Execute {
            registrar_acc_info: _registrar_acc_info,
            token_program_acc_info: _token_program_acc_info,
            is_create: _is_create,
        } = cfg
        {
            pool_token_acc_info = Some(next_account_info(acc_infos)?);
            registry_vault_acc_infos = {
                let mut infos = vec![next_account_info(acc_infos)?];
                if is_mega {
                    infos.push(next_account_info(acc_infos)?);
                }
                Some(infos)
            };
            registry_signer_acc_info = Some(next_account_info(acc_infos)?);
            token_program_acc_info = Some(_token_program_acc_info);

            let nonce = Registrar::unpack(&_registrar_acc_info.try_borrow_data()?)?.nonce;
            signer_seeds = Some((*_registrar_acc_info.key, nonce));
            is_create = _is_create;
        }

        let (pool, mega_pool) = {
            if is_mega {
                let pool = PoolAccounts {
                    pool_program_id_acc_info,
                    pool_acc_info,
                    pool_tok_mint_acc_info,
                    pool_asset_vault_acc_infos,
                    pool_vault_authority_acc_info,
                    retbuf_acc_info,
                    retbuf_program_acc_info,
                    pool_token_acc_info: None,
                    registry_vault_acc_infos: None,
                    registry_signer_acc_info: None,
                    token_program_acc_info: None,
                    signer_seeds,
                };
                let mega_pool = PoolAccounts {
                    pool_program_id_acc_info: pool_program_id_acc_info,
                    pool_acc_info: mega_pool_acc_info,
                    pool_tok_mint_acc_info: mega_pool_tok_mint_acc_info,
                    pool_asset_vault_acc_infos: mega_pool_asset_vault_acc_infos,
                    pool_vault_authority_acc_info: mega_pool_vault_authority_acc_info,
                    retbuf_acc_info,
                    retbuf_program_acc_info,
                    pool_token_acc_info,
                    registry_vault_acc_infos,
                    registry_signer_acc_info,
                    token_program_acc_info,
                    signer_seeds,
                };
                (pool, mega_pool)
            } else {
                let pool = PoolAccounts {
                    pool_program_id_acc_info,
                    pool_acc_info,
                    pool_tok_mint_acc_info,
                    pool_asset_vault_acc_infos,
                    pool_vault_authority_acc_info,
                    retbuf_acc_info,
                    retbuf_program_acc_info,
                    pool_token_acc_info,
                    registry_vault_acc_infos,
                    registry_signer_acc_info,
                    token_program_acc_info,
                    signer_seeds,
                };
                let mega_pool = PoolAccounts {
                    pool_program_id_acc_info: pool_program_id_acc_info,
                    pool_acc_info: mega_pool_acc_info,
                    pool_tok_mint_acc_info: mega_pool_tok_mint_acc_info,
                    pool_asset_vault_acc_infos: mega_pool_asset_vault_acc_infos,
                    pool_vault_authority_acc_info: mega_pool_vault_authority_acc_info,
                    retbuf_acc_info,
                    retbuf_program_acc_info,
                    pool_token_acc_info: None,
                    registry_vault_acc_infos: None,
                    registry_signer_acc_info: None,
                    token_program_acc_info: None,
                    signer_seeds,
                };
                (pool, mega_pool)
            }
        };

        // CPI is expensive. Don't bother fetching both baskets. Just pick the
        // one that's needed (i.e. for the create/redeem invocation) and live
        // with the rounding error for everything else (e.g., when estimating
        // if an Entity is activated or not).
        let prices = match is_create {
            false => PoolPrices::new(
                pool.get_basket(PoolAction::Redeem(1))?,
                mega_pool.get_basket(PoolAction::Redeem(1))?,
            ),
            true => PoolPrices::new(
                pool.get_basket(PoolAction::Create(1))?,
                mega_pool.get_basket(PoolAction::Create(1))?,
            ),
        };

        Ok(Pool {
            accounts: pool,
            mega_accounts: mega_pool,
            is_mega,
            prices,
        })
    }
}

#[derive(Clone)]
pub struct PoolAccounts<'a, 'b> {
    // Common accounts.
    pub pool_acc_info: &'a AccountInfo<'b>,
    pub pool_tok_mint_acc_info: &'a AccountInfo<'b>,
    pub pool_asset_vault_acc_infos: Vec<&'a AccountInfo<'b>>,
    pub pool_vault_authority_acc_info: &'a AccountInfo<'b>,
    pub pool_program_id_acc_info: &'a AccountInfo<'b>,
    pub retbuf_acc_info: &'a AccountInfo<'b>,
    pub retbuf_program_acc_info: &'a AccountInfo<'b>,
    // `execute` only.
    pub pool_token_acc_info: Option<&'a AccountInfo<'b>>,
    pub registry_vault_acc_infos: Option<Vec<&'a AccountInfo<'b>>>,
    pub registry_signer_acc_info: Option<&'a AccountInfo<'b>>,
    pub token_program_acc_info: Option<&'a AccountInfo<'b>>,
    // Misc.
    pub signer_seeds: Option<(Pubkey, u8)>,
}

impl<'a, 'b> PoolAccounts<'a, 'b> {
    #[inline(always)]
    pub fn create(&self, spt_amount: u64) -> Result<(), RegistryError> {
        self.execute(PoolAction::Create(spt_amount))
    }

    #[inline(always)]
    pub fn redeem(&self, spt_amount: u64) -> Result<(), RegistryError> {
        self.execute(PoolAction::Redeem(spt_amount))
    }

    pub fn execute(&self, action: PoolAction) -> Result<(), RegistryError> {
        let instr = serum_stake::instruction::execute(
            self.pool_program_id_acc_info.key,
            self.pool_acc_info.key,
            self.pool_tok_mint_acc_info.key,
            self.pool_asset_vault_acc_infos
                .iter()
                .map(|acc_info| acc_info.key)
                .collect(),
            self.pool_vault_authority_acc_info.key,
            self.pool_token_acc_info.unwrap().key,
            self.registry_vault_acc_infos
                .as_ref()
                .unwrap()
                .iter()
                .map(|i| i.key)
                .collect(),
            self.registry_signer_acc_info.unwrap().key,
            action,
        );
        let (pk, nonce) = self.signer_seeds.expect("transact must have signer seeds");
        let signer_seeds = vault::signer_seeds(&pk, &nonce);
        solana_sdk::program::invoke_signed(&instr, &self.execute_acc_infos(), &[&signer_seeds])?;
        Ok(())
    }

    pub fn get_basket(&self, action: PoolAction) -> Result<Basket, RegistryError> {
        let instr = serum_stake::instruction::get_basket(
            self.pool_program_id_acc_info.key,
            self.pool_acc_info.key,
            self.pool_tok_mint_acc_info.key,
            self.pool_asset_vault_acc_infos
                .iter()
                .map(|acc_info| acc_info.key)
                .collect(),
            self.pool_vault_authority_acc_info.key,
            self.retbuf_acc_info.key,
            action,
        );
        let mut acc_infos = vec![
            self.pool_program_id_acc_info.clone(),
            self.pool_acc_info.clone(),
            self.pool_tok_mint_acc_info.clone(),
        ];
        for acc_info in self.pool_asset_vault_acc_infos.clone() {
            acc_infos.push(acc_info.clone());
        }
        acc_infos.extend_from_slice(&[
            self.pool_vault_authority_acc_info.clone(),
            self.retbuf_acc_info.clone().clone(),
            self.retbuf_program_acc_info.clone(),
        ]);
        solana_sdk::program::invoke(&instr, &acc_infos)?;
        let mut data: &[u8] = &self.retbuf_acc_info.try_borrow_data()?;
        Basket::deserialize(&mut data).map_err(|_| RegistryErrorCode::RetbufError.into())
    }
}

impl<'a, 'b> PoolAccounts<'a, 'b> {
    #[inline(never)]
    fn execute_acc_infos(&self) -> Vec<AccountInfo<'b>> {
        let mut acc_infos = vec![
            self.pool_acc_info.clone(),
            self.pool_tok_mint_acc_info.clone(),
        ];
        acc_infos.extend_from_slice(
            self.pool_asset_vault_acc_infos
                .clone()
                .into_iter()
                .map(|i| i.clone())
                .collect::<Vec<_>>()
                .as_slice(),
        );
        acc_infos.extend_from_slice(&[
            self.pool_vault_authority_acc_info.clone(),
            self.pool_token_acc_info.unwrap().clone(),
        ]);
        acc_infos.extend_from_slice(
            self.registry_vault_acc_infos
                .clone()
                .unwrap()
                .into_iter()
                .map(|i| i.clone())
                .collect::<Vec<_>>()
                .as_slice(),
        );
        acc_infos.extend_from_slice(&[
            self.registry_signer_acc_info.unwrap().clone(),
            self.token_program_acc_info.unwrap().clone(),
            self.pool_program_id_acc_info.clone(),
        ]);

        acc_infos
    }
}

pub enum PoolConfig<'a, 'b> {
    Execute {
        registrar_acc_info: &'a AccountInfo<'b>,
        token_program_acc_info: &'a AccountInfo<'b>,
        is_create: bool,
    },
    GetBasket,
}

// TODO: rename: pool_check_redeem.
pub fn pool_check(
    program_id: &Pubkey,
    pool: &Pool,
    registrar_acc_info: &AccountInfo,
    registrar: &Registrar,
    member: &Member,
) -> Result<(), RegistryError> {
    _pool_check(
        program_id,
        pool,
        registrar_acc_info,
        registrar,
        Some(member),
    )?;
    Ok(())
}

pub fn pool_check_get_basket(
    program_id: &Pubkey,
    pool: &Pool,
    registrar_acc_info: &AccountInfo,
    registrar: &Registrar,
) -> Result<(), RegistryError> {
    let _ = _pool_check(program_id, pool, registrar_acc_info, registrar, None)?;
    Ok(())
}

pub fn pool_check_create(
    program_id: &Pubkey,
    pool: &Pool,
    registrar_acc_info: &AccountInfo,
    registrar: &Registrar,
    member: &Member,
) -> Result<TokenAccount, RegistryError> {
    Ok(_pool_check(
        program_id,
        pool,
        registrar_acc_info,
        registrar,
        Some(member),
    )?
    .expect("must have token"))
}

fn _pool_check(
    program_id: &Pubkey,
    pool: &Pool,
    registrar_acc_info: &AccountInfo,
    registrar: &Registrar,
    member: Option<&Member>,
) -> Result<Option<TokenAccount>, RegistryError> {
    // Check registry signer.
    if let Some(registry_signer_acc_info) = pool.registry_signer_acc_info {
        let r_signer = Pubkey::create_program_address(
            &vault::signer_seeds(registrar_acc_info.key, &registrar.nonce),
            program_id,
        )
        .map_err(|_| RegistryErrorCode::InvalidVaultAuthority)?;
        if registry_signer_acc_info.key != &r_signer {
            return Err(RegistryErrorCode::InvalidVaultAuthority)?;
        }
    }
    // Check pool program id.
    if registrar.pool_program_id != *pool.pool_program_id_acc_info.key {
        return Err(RegistryErrorCode::PoolProgramIdMismatch)?;
    }
    // Check pool accounts.
    if registrar.pool != *pool.accounts.pool_acc_info.key {
        return Err(RegistryErrorCode::PoolMismatch)?;
    }
    if registrar.mega_pool != *pool.mega_accounts.pool_acc_info.key {
        return Err(RegistryErrorCode::MegaPoolMismatch)?;
    }
    // Check is_mega.
    if pool.is_mega && registrar.mega_pool != *pool.pool_acc_info.key {
        return Err(RegistryErrorCode::PoolMismatch)?;
    }
    if !pool.is_mega && registrar.pool != *pool.pool_acc_info.key {
        return Err(RegistryErrorCode::PoolMismatch)?;
    }
    // Check retbuf.
    if shared_mem::ID != *pool.retbuf_program_acc_info.key {
        return Err(RegistryErrorCode::SharedMemoryMismatch)?;
    }
    // Check pool token.
    if let Some(pool_token) = pool.pool_token_acc_info {
        let member = member.expect("member must be provided");
        return Ok(Some(pool_token_check(
            pool.registry_signer_acc_info.unwrap(),
            pool_token,
            member,
        )?));
    }

    // Assumes the rest of the checks are done by the pool program/framework.

    Ok(None)
}

// Pool token must be owned by the registry with the member account's
// beneficiary as delegate.
pub fn pool_token_check(
    registry_signer_acc_info: &AccountInfo,
    pool_token: &AccountInfo,
    member: &Member,
) -> Result<TokenAccount, RegistryError> {
    let token = TokenAccount::unpack(&pool_token.try_borrow_data()?)?;
    if token.owner != *registry_signer_acc_info.key {
        return Err(RegistryErrorCode::InvalidStakeTokenOwner)?;
    }

    if token.delegate != COption::Some(member.beneficiary) {
        return Err(RegistryErrorCode::InvalidStakeTokenDelegate)?;
    }

    return Ok(token);
}
