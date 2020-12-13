use std::convert::TryInto;

use crate::next_account_infos;
use serum_pool_schema::{
    Address, Basket, PoolRequestInner, PoolState, FEE_RATE_DENOMINATOR, MIN_FEE_RATE,
};
use solana_program;
use solana_program::account_info::next_account_info;
use solana_program::instruction::{AccountMeta, Instruction};
use solana_program::program;
use solana_program::program_option::COption;
use solana_program::program_pack::Pack;
use solana_program::sysvar::{rent, Sysvar};
use solana_program::{account_info::AccountInfo, msg, program_error::ProgramError, pubkey::Pubkey};
use spl_token::state::{Account as TokenAccount, Mint};
use std::cmp::{max, min};

pub struct PoolContext<'a, 'b> {
    pub program_id: &'a Pubkey,

    /// Account that holds the `PoolState`.
    pub pool_account: &'a AccountInfo<'b>,

    /// Token mint account for the pool token.
    pub pool_token_mint: &'a AccountInfo<'b>,
    /// Token accounts for each of the assets owned by the pool.
    pub pool_vault_accounts: &'a [AccountInfo<'b>],
    /// Signer for `pool_token_mint` and `pool_vault_accounts`.
    pub pool_authority: &'a AccountInfo<'b>,

    /// Present for `Initialize` requests.
    pub rent: Option<rent::Rent>,

    /// Present for `GetBasket` requests.
    pub retbuf: Option<RetbufAccounts<'a, 'b>>,

    /// Present for `Execute` requests.
    pub user_accounts: Option<UserAccounts<'a, 'b>>,

    /// Present for `Execute` requests.
    pub fee_accounts: Option<FeeAccounts<'a, 'b>>,

    /// Present for `Execute` requests.
    pub spl_token_program: Option<&'a AccountInfo<'b>>,

    /// Accounts from `PoolState::account_params`. Present for `GetBasket` and `Transact` requests.
    pub account_params: Option<&'a [AccountInfo<'b>]>,

    /// Any additional accounts that were passed into the instruction.
    pub custom_accounts: &'a [AccountInfo<'b>],
}

pub struct UserAccounts<'a, 'b> {
    pub pool_token_account: &'a AccountInfo<'b>,
    pub asset_accounts: &'a [AccountInfo<'b>],
    pub authority: &'a AccountInfo<'b>,
}

pub struct FeeAccounts<'a, 'b> {
    pub lqd_fee_account: &'a AccountInfo<'b>,
    pub initializer_fee_account: &'a AccountInfo<'b>,
    pub referrer_fee_account: &'a AccountInfo<'b>,
}

pub struct RetbufAccounts<'a, 'b> {
    pub account: &'a AccountInfo<'b>,
    pub program: &'a AccountInfo<'b>,
}

impl<'a, 'b> PoolContext<'a, 'b> {
    pub fn new(
        program_id: &'a Pubkey,
        accounts: &'a [AccountInfo<'b>],
        state: &PoolState,
        request: &PoolRequestInner,
    ) -> Result<Self, ProgramError> {
        let accounts_iter = &mut accounts.into_iter();

        let pool_account = next_account_info(accounts_iter)?;
        let pool_token_mint = next_account_info(accounts_iter)?;
        let pool_vault_accounts = next_account_infos(accounts_iter, state.assets.len())?;
        let pool_authority = next_account_info(accounts_iter)?;
        let mut context = PoolContext {
            program_id,
            pool_account,
            pool_token_mint,
            pool_vault_accounts,
            pool_authority,
            rent: None,
            retbuf: None,
            user_accounts: None,
            fee_accounts: None,
            spl_token_program: None,
            account_params: None,
            custom_accounts: &[],
        };

        check_account_address(context.pool_token_mint, &state.pool_token_mint)?;
        check_mint_minter(context.pool_token_mint, state.vault_signer.as_ref())?;
        for (asset_info, vault_account) in
            state.assets.iter().zip(context.pool_vault_accounts.iter())
        {
            check_account_address(vault_account, &asset_info.vault_address)?;
            check_token_account(
                vault_account,
                asset_info.mint.as_ref(),
                Some(state.vault_signer.as_ref()),
            )?;
        }
        check_account_address(context.pool_authority, &state.vault_signer)?;

        match request {
            PoolRequestInner::GetBasket(_) => {
                let retbuf_account = next_account_info(accounts_iter)?;
                let retbuf_program = next_account_info(accounts_iter)?;
                context.retbuf = Some(RetbufAccounts::new(retbuf_account, retbuf_program)?);
                context.account_params = Some(next_account_infos(
                    accounts_iter,
                    state.account_params.len(),
                )?);
            }
            PoolRequestInner::Execute(_) => {
                let pool_token_account = next_account_info(accounts_iter)?;
                let asset_accounts = next_account_infos(accounts_iter, state.assets.len())?;
                let authority = next_account_info(accounts_iter)?;
                let lqd_fee_account = next_account_info(accounts_iter)?;
                let initializer_fee_account = next_account_info(accounts_iter)?;
                let referrer_fee_account = next_account_info(accounts_iter)?;
                context.user_accounts = Some(UserAccounts::new(
                    state,
                    pool_token_account,
                    asset_accounts,
                    authority,
                )?);
                context.fee_accounts = Some(FeeAccounts::new(
                    state,
                    lqd_fee_account,
                    initializer_fee_account,
                    referrer_fee_account,
                )?);
                context.spl_token_program = Some(next_account_info(accounts_iter)?);
                context.account_params = Some(next_account_infos(
                    accounts_iter,
                    state.account_params.len(),
                )?);
            }
            PoolRequestInner::Initialize(_) => {
                let lqd_fee_account = next_account_info(accounts_iter)?;
                let initializer_fee_account = next_account_info(accounts_iter)?;
                let rent_sysvar_account = next_account_info(accounts_iter)?;
                context.fee_accounts = Some(FeeAccounts::new(
                    state,
                    lqd_fee_account,
                    initializer_fee_account,
                    lqd_fee_account,
                )?);
                if rent_sysvar_account.key != &rent::ID {
                    msg!("Incorrect rent sysvar account");
                    return Err(ProgramError::InvalidArgument);
                }
                let rent = rent::Rent::from_account_info(rent_sysvar_account).map_err(|_| {
                    msg!("Failed to deserialize rent sysvar");
                    ProgramError::InvalidArgument
                })?;
                context.rent = Some(rent);
            }
        }

        if let Some(spl_token_program) = context.spl_token_program {
            if spl_token_program.key != &spl_token::ID {
                msg!("Incorrect spl-token program ID");
                return Err(ProgramError::InvalidArgument);
            }
        }

        if let Some(account_params) = context.account_params {
            for (param_desc, account_info) in state.account_params.iter().zip(account_params.iter())
            {
                check_account_address(account_info, &param_desc.address)?;
            }
        }

        context.custom_accounts = accounts_iter.as_slice();

        Ok(context)
    }
}

impl<'a, 'b> UserAccounts<'a, 'b> {
    pub fn new(
        state: &PoolState,
        pool_token_account: &'a AccountInfo<'b>,
        asset_accounts: &'a [AccountInfo<'b>],
        authority: &'a AccountInfo<'b>,
    ) -> Result<Self, ProgramError> {
        check_token_account(pool_token_account, state.pool_token_mint.as_ref(), None)?;
        for (asset_info, account) in state.assets.iter().zip(asset_accounts.iter()) {
            check_token_account(account, asset_info.mint.as_ref(), None)?;
        }
        Ok(UserAccounts {
            pool_token_account,
            asset_accounts,
            authority,
        })
    }
}

impl<'a, 'b> FeeAccounts<'a, 'b> {
    pub fn new(
        state: &PoolState,
        lqd_fee_account: &'a AccountInfo<'b>,
        initializer_fee_account: &'a AccountInfo<'b>,
        referrer_fee_account: &'a AccountInfo<'b>,
    ) -> Result<Self, ProgramError> {
        check_account_address(lqd_fee_account, &state.lqd_fee_vault)?;
        check_account_address(initializer_fee_account, &state.initializer_fee_vault)?;
        check_token_account(
            lqd_fee_account,
            state.pool_token_mint.as_ref(),
            Some(&serum_pool_schema::fee_owner::ID),
        )?;
        check_token_account(
            initializer_fee_account,
            state.pool_token_mint.as_ref(),
            None,
        )?;
        check_token_account(referrer_fee_account, state.pool_token_mint.as_ref(), None)?;
        Ok(FeeAccounts {
            lqd_fee_account,
            initializer_fee_account,
            referrer_fee_account,
        })
    }
}

impl<'a, 'b> RetbufAccounts<'a, 'b> {
    pub fn new(
        account: &'a AccountInfo<'b>,
        program: &'a AccountInfo<'b>,
    ) -> Result<Self, ProgramError> {
        if account.owner != program.key {
            msg!("Incorrect retbuf account owner");
            return Err(ProgramError::IncorrectProgramId);
        }
        Ok(RetbufAccounts { account, program })
    }

    // data is a Vec whose first 8 bytes are the little-endian offset at which to
    // write the remaining bytes
    pub(crate) fn write_data(&self, data: Vec<u8>) -> Result<(), ProgramError> {
        msg!(&base64::encode(&data[8..]));
        let instruction = Instruction {
            program_id: *self.program.key,
            accounts: vec![AccountMeta::new(*self.account.key, false)],
            data,
        };
        program::invoke(&instruction, &[self.account.clone(), self.program.clone()])?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fees {
    pub lqd_fee: u64,
    pub initializer_fee: u64,
    pub referrer_fee: u64,
}

impl Fees {
    pub fn total_fee(&self) -> u64 {
        self.lqd_fee + self.initializer_fee + self.referrer_fee
    }

    pub fn from_fee_rate_and_tokens(fee_rate: u32, tokens: u64) -> Result<Self, ProgramError> {
        if fee_rate < MIN_FEE_RATE || fee_rate >= FEE_RATE_DENOMINATOR {
            msg!("Invalid fee");
            Err(ProgramError::InvalidArgument)
        } else if tokens == 0 {
            Ok(Fees {
                lqd_fee: 0,
                referrer_fee: 0,
                initializer_fee: 0,
            })
        } else {
            let total_fee = (((tokens as u128) * (fee_rate as u128) - 1)
                / FEE_RATE_DENOMINATOR as u128
                + 1) as u64;
            assert!(total_fee <= tokens);
            let lqd_fee = max(
                total_fee.checked_mul(2).unwrap() / 5,
                (tokens - 1) / 10000 + 1,
            );
            assert!(lqd_fee <= total_fee);
            let referrer_fee = min(lqd_fee / 2, total_fee - lqd_fee);
            assert!(lqd_fee.checked_add(referrer_fee).unwrap() <= total_fee);
            let initializer_fee = total_fee
                .checked_sub(lqd_fee)
                .unwrap()
                .checked_sub(referrer_fee)
                .unwrap();
            assert!(
                lqd_fee
                    .checked_add(referrer_fee)
                    .unwrap()
                    .checked_add(initializer_fee)
                    .unwrap()
                    <= tokens
            );

            Ok(Fees {
                lqd_fee,
                referrer_fee,
                initializer_fee,
            })
        }
    }
}

impl<'a, 'b> PoolContext<'a, 'b> {
    pub(crate) fn derive_vault_authority(&self, state: &PoolState) -> Result<Pubkey, ProgramError> {
        let seeds = &[self.pool_account.key.as_ref(), &[state.vault_signer_nonce]];
        Ok(
            Pubkey::create_program_address(seeds, self.program_id).map_err(|e| {
                msg!("Invalid vault signer nonce");
                e
            })?,
        )
    }

    pub fn check_rent_exemption(&self, account: &AccountInfo) -> Result<(), ProgramError> {
        let rent = self.rent.ok_or_else(|| {
            msg!("Rent parameters not present");
            ProgramError::InvalidArgument
        })?;
        let data_len = account.try_data_len()?;
        let lamports = account.try_lamports()?;
        if rent.is_exempt(lamports, data_len as usize) {
            Ok(())
        } else {
            msg!("Account is not rent exempt");
            Err(ProgramError::InvalidArgument)
        }
    }

    /// Total number of pool tokens currently in existence.
    pub fn total_pool_tokens(&self) -> Result<u64, ProgramError> {
        let mint = Mint::unpack(&self.pool_token_mint.try_borrow_data()?)?;
        Ok(mint.supply)
    }

    /// For each token in `PoolState::assets`, the quantity of that token currently
    /// held by the pool.
    pub fn pool_asset_quantities(&self) -> Result<Vec<u64>, ProgramError> {
        self.pool_vault_accounts
            .iter()
            .map(|account| -> Result<u64, ProgramError> {
                let token_account = TokenAccount::unpack(&account.try_borrow_data()?)?;
                Ok(token_account.amount)
            })
            .collect()
    }

    /// Computes a basket by dividing the current contents of the pool vaults by the
    /// number of outstanding pool tokens.
    pub fn get_simple_basket(
        &self,
        pool_tokens_requested: u64,
        round_up: bool,
    ) -> Result<Basket, ProgramError> {
        let total_pool_tokens = self.total_pool_tokens()?;
        if total_pool_tokens == 0 {
            msg!("Pool is empty");
            return Err(ProgramError::InvalidArgument);
        }
        let basket_quantities: Option<Vec<i64>> = self
            .pool_asset_quantities()?
            .iter()
            .map(|pool_quantity| {
                (*pool_quantity as u128)
                    .checked_mul(pool_tokens_requested as u128)?
                    .checked_add(if round_up {
                        total_pool_tokens.checked_sub(1)?
                    } else {
                        0
                    } as u128)?
                    .checked_div(total_pool_tokens as u128)?
                    .try_into()
                    .ok()
            })
            .collect();
        Ok(Basket {
            quantities: basket_quantities.ok_or_else(|| {
                msg!("Per-share quantity doesn't fit into an i64");
                ProgramError::InvalidArgument
            })?,
        })
    }

    /// Computes the fees to charge for creating or redeeming pool tokens.
    pub fn get_fees(&self, state: &PoolState, pool_tokens: u64) -> Result<Fees, ProgramError> {
        let mut fees = Fees::from_fee_rate_and_tokens(state.fee_rate, pool_tokens)?;
        if let Some(user_accounts) = &self.user_accounts {
            let user_key = user_accounts.pool_token_account.key;
            if let Some(fee_accounts) = &self.fee_accounts {
                if fee_accounts.lqd_fee_account.key == user_key {
                    fees.lqd_fee = 0;
                    fees.initializer_fee = 0;
                    fees.referrer_fee = 0;
                }
                if fee_accounts.initializer_fee_account.key == user_key {
                    fees.initializer_fee = 0;
                }
                if fee_accounts.referrer_fee_account.key == user_key {
                    fees.referrer_fee = 0;
                }
            }
        }
        Ok(fees)
    }

    /// Transfers basket tokens from the user to the pool.
    pub fn transfer_basket_from_user(&self, basket: &Basket) -> Result<(), ProgramError> {
        let user_accounts = self
            .user_accounts
            .as_ref()
            .ok_or(ProgramError::InvalidArgument)?;
        let pool_vault_accounts = self.pool_vault_accounts;
        let spl_token_program = self
            .spl_token_program
            .ok_or(ProgramError::InvalidArgument)?;

        let zipped_iter = basket
            .quantities
            .iter()
            .zip(user_accounts.asset_accounts.iter())
            .zip(pool_vault_accounts.iter());

        for ((&input_qty, user_asset_account), pool_vault_account) in zipped_iter {
            let source_pubkey = user_asset_account.key;
            let destination_pubkey = pool_vault_account.key;
            let authority_pubkey = user_accounts.authority.key;
            let signer_pubkeys = &[];

            let instruction = spl_token::instruction::transfer(
                &spl_token::ID,
                source_pubkey,
                destination_pubkey,
                authority_pubkey,
                signer_pubkeys,
                input_qty
                    .try_into()
                    .or(Err(ProgramError::InvalidArgument))?,
            )?;

            let account_infos = &[
                user_asset_account.clone(),
                pool_vault_account.clone(),
                user_accounts.authority.clone(),
                spl_token_program.clone(),
            ];

            program::invoke(&instruction, account_infos)?;
        }

        Ok(())
    }

    /// Mints pool tokens to the requester for a creation request.
    ///
    /// Fees are deducted and sent to the fee account before the remainder is sent
    /// to the user.
    pub fn mint_tokens(&self, state: &PoolState, quantity: u64) -> Result<(), ProgramError> {
        let fees = self.get_fees(state, quantity)?;
        let remainder = quantity - fees.total_fee();

        let user_accounts = self
            .user_accounts
            .as_ref()
            .ok_or(ProgramError::InvalidArgument)?;
        let fee_accounts = self
            .fee_accounts
            .as_ref()
            .ok_or(ProgramError::InvalidArgument)?;
        let spl_token_program = self
            .spl_token_program
            .ok_or(ProgramError::InvalidArgument)?;

        for (account, quantity) in &[
            (fee_accounts.lqd_fee_account, fees.lqd_fee),
            (fee_accounts.initializer_fee_account, fees.initializer_fee),
            (fee_accounts.referrer_fee_account, fees.referrer_fee),
            (user_accounts.pool_token_account, remainder),
        ] {
            let account = *account;
            let quantity = *quantity;
            if quantity > 0 {
                let mint_pubkey = self.pool_token_mint.key;
                let account_pubkey = account.key;
                let owner_pubkey = self.pool_authority.key;
                let signer_pubkeys = &[];
                let instruction = spl_token::instruction::mint_to(
                    &spl_token::ID,
                    mint_pubkey,
                    account_pubkey,
                    owner_pubkey,
                    signer_pubkeys,
                    quantity,
                )?;
                let account_infos = &[
                    account.clone(),
                    self.pool_token_mint.clone(),
                    self.pool_authority.clone(),
                    spl_token_program.clone(),
                ];
                program::invoke_signed(
                    &instruction,
                    account_infos,
                    &[&[self.pool_account.key.as_ref(), &[state.vault_signer_nonce]]],
                )?;
            }
        }

        Ok(())
    }

    /// Burns pool tokens from the requester for a redemption request.
    pub(crate) fn burn_tokens_and_collect_fees(
        &self,
        redemption_size: u64,
        fees: Fees,
    ) -> Result<(), ProgramError> {
        let user_accounts = self
            .user_accounts
            .as_ref()
            .ok_or(ProgramError::InvalidArgument)?;
        let fee_accounts = self
            .fee_accounts
            .as_ref()
            .ok_or(ProgramError::InvalidArgument)?;
        let spl_token_program = self
            .spl_token_program
            .ok_or(ProgramError::InvalidArgument)?;

        for (account, quantity) in &[
            (fee_accounts.lqd_fee_account, fees.lqd_fee),
            (fee_accounts.initializer_fee_account, fees.initializer_fee),
            (fee_accounts.referrer_fee_account, fees.referrer_fee),
        ] {
            let account = *account;
            let quantity = *quantity;
            if quantity > 0 {
                let source_pubkey = user_accounts.pool_token_account.key;
                let destination_pubkey = account.key;
                let authority_pubkey = user_accounts.authority.key;
                let signer_pubkeys = &[];

                let instruction = spl_token::instruction::transfer(
                    &spl_token::ID,
                    source_pubkey,
                    destination_pubkey,
                    authority_pubkey,
                    signer_pubkeys,
                    quantity,
                )?;

                let account_infos = &[
                    user_accounts.pool_token_account.clone(),
                    account.clone(),
                    user_accounts.authority.clone(),
                    spl_token_program.clone(),
                ];

                program::invoke(&instruction, account_infos)?;
            }
        }

        {
            let mint_pubkey = self.pool_token_mint.key;
            let account_pubkey = user_accounts.pool_token_account.key;
            let authority_pubkey = user_accounts.authority.key;
            let signer_pubkeys = &[];

            let instruction = spl_token::instruction::burn(
                &spl_token::ID,
                account_pubkey,
                mint_pubkey,
                authority_pubkey,
                signer_pubkeys,
                redemption_size,
            )?;

            let account_infos = &[
                self.pool_token_mint.clone(),
                user_accounts.pool_token_account.clone(),
                user_accounts.authority.clone(),
                spl_token_program.clone(),
            ];

            program::invoke(&instruction, account_infos)?;
        }

        Ok(())
    }

    /// Transfers basket tokens from the pool to the user.
    pub fn transfer_basket_to_user(
        &self,
        state: &PoolState,
        basket: &Basket,
    ) -> Result<(), ProgramError> {
        let user_accounts = self
            .user_accounts
            .as_ref()
            .ok_or(ProgramError::InvalidArgument)?;
        let pool_vault_accounts = self.pool_vault_accounts;
        let spl_token_program = self
            .spl_token_program
            .ok_or(ProgramError::InvalidArgument)?;

        let zipped_iter = basket
            .quantities
            .iter()
            .zip(user_accounts.asset_accounts.iter())
            .zip(pool_vault_accounts.iter());

        for ((&output_qty, user_asset_account), pool_vault_account) in zipped_iter {
            let source_pubkey = pool_vault_account.key;
            let destination_pubkey = user_asset_account.key;
            let authority_pubkey = self.pool_authority.key;
            let signer_pubkeys = &[];

            let instruction = spl_token::instruction::transfer(
                &spl_token::ID,
                source_pubkey,
                destination_pubkey,
                authority_pubkey,
                signer_pubkeys,
                output_qty
                    .try_into()
                    .or(Err(ProgramError::InvalidArgument))?,
            )?;

            let account_infos = &[
                user_asset_account.clone(),
                pool_vault_account.clone(),
                self.pool_authority.clone(),
                spl_token_program.clone(),
            ];

            program::invoke_signed(
                &instruction,
                account_infos,
                &[&[self.pool_account.key.as_ref(), &[state.vault_signer_nonce]]],
            )?;
        }

        Ok(())
    }
}

fn check_account_address(account: &AccountInfo, address: &Address) -> Result<(), ProgramError> {
    if account.key != address.as_ref() {
        msg!("Incorrect account address");
        return Err(ProgramError::InvalidArgument);
    }
    Ok(())
}

fn check_mint_minter(account: &AccountInfo, mint_authority: &Pubkey) -> Result<(), ProgramError> {
    if account.owner != &spl_token::ID {
        msg!("Account not owned by spl-token program");
        return Err(ProgramError::IncorrectProgramId);
    }
    let mint = Mint::unpack(&account.try_borrow_data()?)?;
    if mint.mint_authority != COption::Some(*mint_authority) {
        msg!("Incorrect mint authority");
        return Err(ProgramError::InvalidArgument);
    }
    Ok(())
}

fn check_token_account(
    account: &AccountInfo,
    mint: &Pubkey,
    authority: Option<&Pubkey>,
) -> Result<(), ProgramError> {
    if account.owner != &spl_token::ID {
        msg!("Account not owned by spl-token program");
        return Err(ProgramError::IncorrectProgramId);
    }
    let token_account = TokenAccount::unpack(&account.try_borrow_data()?)?;
    if &token_account.mint != mint {
        msg!("Incorrect mint");
        return Err(ProgramError::InvalidArgument);
    }
    if let Some(authority) = authority {
        if &token_account.owner != authority && token_account.delegate != COption::Some(*authority)
        {
            msg!("Incorrect spl-token account owner");
            return Err(ProgramError::InvalidArgument);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_fees() {
        assert_eq!(
            Fees::from_fee_rate_and_tokens(2500, 100_000).unwrap(),
            Fees {
                lqd_fee: 100,
                initializer_fee: 100,
                referrer_fee: 50,
            }
        );
    }

    #[test]
    fn test_get_fees_small_size() {
        assert_eq!(
            Fees::from_fee_rate_and_tokens(2500, 10).unwrap(),
            Fees {
                lqd_fee: 1,
                initializer_fee: 0,
                referrer_fee: 0,
            }
        );
    }

    #[test]
    fn test_get_fees_min_rate() {
        assert_eq!(
            Fees::from_fee_rate_and_tokens(MIN_FEE_RATE, 100_000).unwrap(),
            Fees {
                lqd_fee: 10,
                initializer_fee: 0,
                referrer_fee: 5,
            }
        );
    }

    #[test]
    fn test_get_fees_min_rate_small_size() {
        assert_eq!(
            Fees::from_fee_rate_and_tokens(MIN_FEE_RATE, 100).unwrap(),
            Fees {
                lqd_fee: 1,
                initializer_fee: 0,
                referrer_fee: 0,
            }
        );
    }

    #[test]
    fn test_get_fees_zero_size() {
        assert_eq!(
            Fees::from_fee_rate_and_tokens(MIN_FEE_RATE, 0).unwrap(),
            Fees {
                lqd_fee: 0,
                initializer_fee: 0,
                referrer_fee: 0,
            }
        );
    }

    #[test]
    fn test_get_fees_max_rate() {
        assert_eq!(
            Fees::from_fee_rate_and_tokens(999_999, 100_000).unwrap(),
            Fees {
                lqd_fee: 40_000,
                initializer_fee: 40_000,
                referrer_fee: 20_000,
            }
        );
    }

    #[test]
    fn test_get_fees_rate_too_low() {
        assert!(Fees::from_fee_rate_and_tokens(149, 100_000).is_err());
    }

    #[test]
    fn test_get_fees_rate_too_high() {
        assert!(Fees::from_fee_rate_and_tokens(1_000_000, 100_000).is_err());
    }
}
