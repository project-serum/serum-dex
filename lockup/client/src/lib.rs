//! Client wraps the solana generated client with Request/Response structs
//! to make usage cleaner and less error prone.

use serum_common::client::rpc;
use serum_lockup::accounts::{Safe, TokenVault, Vesting, Whitelist};
use serum_lockup::client::{Client as InnerClient, ClientError as InnerClientError};
use solana_client_gen::prelude::Signer;
use solana_client_gen::prelude::*;
use solana_client_gen::solana_sdk;
use solana_sdk::instruction::AccountMeta;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Signature;
use spl_token::state::Account as TokenAccount;
use std::convert::Into;
use thiserror::Error;

mod inner;

pub struct Client {
    inner: InnerClient,
}

impl Client {
    pub fn new(inner: InnerClient) -> Self {
        Self { inner }
    }

    pub fn initialize(&self, req: InitializeRequest) -> Result<InitializeResponse, ClientError> {
        inner::create_all_accounts_and_initialize(&self.inner, &req.mint, &req.authority)
            .map_err(Into::into)
    }

    pub fn create_vesting(
        &self,
        req: CreateVestingRequest,
    ) -> Result<CreateVestingResponse, ClientError> {
        let vault = self.safe(&req.safe)?.vault;
        let mint_decimals = 3; // TODO: decide this.
        inner::create_vesting_account(
            &self.inner,
            &req.depositor,
            req.depositor_owner,
            &req.safe,
            &vault,
            &self.vault_authority(req.safe)?,
            &req.beneficiary,
            req.end_slot,
            req.period_count,
            req.deposit_amount,
            mint_decimals,
        )
        .map_err(Into::into)
        .map(|r| CreateVestingResponse {
            tx: r.0,
            vesting: r.1.pubkey(),
            mint: r.2,
        })
    }

    pub fn whitelist_add(
        &self,
        req: WhitelistAddRequest,
    ) -> Result<WhitelistAddResponse, ClientError> {
        let WhitelistAddRequest {
            authority,
            safe,
            program,
        } = req;
        let whitelist = self.safe(&safe)?.whitelist;
        let accounts = [
            AccountMeta::new_readonly(authority.pubkey(), true),
            AccountMeta::new_readonly(safe, false),
            AccountMeta::new(whitelist, false),
        ];
        let signers = [self.payer(), authority];
        let tx = self
            .inner
            .whitelist_add_with_signers(&signers, &accounts, program)?;
        Ok(WhitelistAddResponse { tx })
    }

    pub fn whitelist_delete(
        &self,
        req: WhitelistDeleteRequest,
    ) -> Result<WhitelistDeleteResponse, ClientError> {
        let WhitelistDeleteRequest {
            authority,
            safe,
            program,
        } = req;
        let whitelist = self.safe(&safe)?.whitelist;
        let accounts = [
            AccountMeta::new_readonly(authority.pubkey(), true),
            AccountMeta::new_readonly(safe, false),
            AccountMeta::new(whitelist, false),
        ];
        let signers = [self.payer(), authority];
        let tx = self
            .inner
            .whitelist_delete_with_signers(&signers, &accounts, program)?;
        Ok(WhitelistDeleteResponse { tx })
    }

    pub fn whitelist_withdraw(
        &self,
        req: WhitelistWithdrawRequest,
    ) -> Result<WhitelistWithdrawResponse, ClientError> {
        let WhitelistWithdrawRequest {
            beneficiary,
            vesting,
            safe,
            whitelist_program,
            mut relay_accounts,
            vault,
            whitelist_vault,
            whitelist_vault_authority,
            delegate_amount,
            relay_data,
        } = req;
        let whitelist = self.safe(&safe)?.whitelist;
        let mut accounts = vec![
            AccountMeta::new_readonly(beneficiary.pubkey(), true),
            AccountMeta::new(vesting, false),
            AccountMeta::new_readonly(safe, false),
            AccountMeta::new_readonly(self.vault_authority(safe)?, false),
            AccountMeta::new_readonly(whitelist_program, false),
            AccountMeta::new_readonly(whitelist, false),
            // Below are relay accounts.
            AccountMeta::new(vault, false),
            AccountMeta::new(whitelist_vault, false),
            AccountMeta::new_readonly(whitelist_vault_authority, false),
            AccountMeta::new_readonly(spl_token::ID, false),
        ];
        accounts.append(&mut relay_accounts);

        let signers = [self.payer(), &beneficiary];

        let tx = self
            .inner
            .whitelist_withdraw_with_signers(&signers, &accounts, delegate_amount, relay_data)
            .unwrap();

        Ok(WhitelistWithdrawResponse { tx })
    }

    pub fn whitelist_deposit(
        &self,
        req: WhitelistDepositRequest,
    ) -> Result<WhitelistDepositResponse, ClientError> {
        let WhitelistDepositRequest {
            beneficiary,
            vesting,
            safe,
            whitelist_program,
            mut relay_accounts,
            vault,
            whitelist_vault,
            whitelist_vault_authority,
            relay_data,
        } = req;
        let whitelist = self.safe(&safe)?.whitelist;
        let mut accounts = vec![
            AccountMeta::new_readonly(beneficiary.pubkey(), true),
            AccountMeta::new(vesting, false),
            AccountMeta::new_readonly(safe, false),
            AccountMeta::new_readonly(self.vault_authority(safe)?, false),
            AccountMeta::new_readonly(whitelist_program, false),
            AccountMeta::new_readonly(whitelist, false),
            // Below are relay accounts.
            AccountMeta::new(vault, false),
            AccountMeta::new(whitelist_vault, false),
            AccountMeta::new_readonly(whitelist_vault_authority, false),
            AccountMeta::new_readonly(spl_token::ID, false),
        ];
        accounts.append(&mut relay_accounts);

        let signers = [self.payer(), &beneficiary];

        let tx = self
            .inner
            .whitelist_deposit_with_signers(&signers, &accounts, relay_data)
            .unwrap();

        Ok(WhitelistDepositResponse { tx })
    }

    pub fn claim(&self, req: ClaimRequest) -> Result<ClaimResponse, ClientError> {
        let ClaimRequest {
            beneficiary,
            safe,
            vesting,
            locked_mint,
            locked_token_account,
        } = req;
        let accounts = [
            AccountMeta::new_readonly(beneficiary.pubkey(), true),
            AccountMeta::new(vesting, false),
            AccountMeta::new_readonly(safe, false),
            AccountMeta::new_readonly(self.vault_authority(safe)?, false),
            AccountMeta::new_readonly(spl_token::ID, false),
            AccountMeta::new(locked_mint, false),
            AccountMeta::new(locked_token_account, false),
        ];
        let signers = [self.payer(), &beneficiary];
        let tx = self.inner.claim_with_signers(&signers, &accounts)?;

        Ok(ClaimResponse { tx })
    }

    pub fn redeem(&self, req: RedeemRequest) -> Result<RedeemResponse, ClientError> {
        let RedeemRequest {
            beneficiary,
            vesting,
            token_account,
            vault,
            safe,
            locked_token_account,
            locked_mint,
            amount,
        } = req;
        let accounts = [
            AccountMeta::new_readonly(beneficiary.pubkey(), true),
            AccountMeta::new(vesting, false),
            AccountMeta::new(token_account, false),
            AccountMeta::new(vault, false),
            AccountMeta::new_readonly(self.vault_authority(safe)?, false),
            AccountMeta::new_readonly(safe, false),
            AccountMeta::new(locked_token_account, false),
            AccountMeta::new(locked_mint, false),
            AccountMeta::new_readonly(spl_token::ID, false),
            AccountMeta::new_readonly(solana_sdk::sysvar::clock::ID, false),
        ];
        let signers = [self.payer(), &beneficiary];
        let tx = self
            .inner
            .redeem_with_signers(&signers, &accounts, amount)?;
        Ok(RedeemResponse { tx })
    }

    pub fn set_authority(
        &self,
        req: SetAuthorityRequest,
    ) -> Result<SetAuthorityResponse, ClientError> {
        let SetAuthorityRequest {
            authority,
            safe,
            new_authority,
        } = req;
        let accounts = [
            AccountMeta::new_readonly(authority.pubkey(), true),
            AccountMeta::new(safe, false),
        ];
        let signers = [&authority, self.payer()];
        let tx = self
            .inner
            .set_authority_with_signers(&signers, &accounts, new_authority)?;
        Ok(SetAuthorityResponse { tx })
    }

    pub fn migrate(&self, req: MigrateRequest) -> Result<MigrateResponse, ClientError> {
        let MigrateRequest {
            authority,
            safe,
            new_token_account,
        } = req;
        let vault = self.safe(&safe)?.vault;
        let accounts = [
            AccountMeta::new_readonly(authority.pubkey(), true),
            AccountMeta::new(safe, false),
            AccountMeta::new(vault, false),
            AccountMeta::new_readonly(self.vault_authority(safe)?, false),
            AccountMeta::new(new_token_account, false),
            AccountMeta::new_readonly(spl_token::ID, false),
        ];
        let signers = [&authority, self.payer()];
        let tx = self.inner.migrate_with_signers(&signers, &accounts)?;

        Ok(MigrateResponse { tx })
    }
}

// Account accessors.
impl Client {
    pub fn safe(&self, address: &Pubkey) -> Result<Safe, ClientError> {
        rpc::get_account::<Safe>(self.inner.rpc(), address).map_err(Into::into)
    }

    pub fn whitelist(&self, safe: &Pubkey) -> Result<Whitelist, ClientError> {
        let safe = rpc::get_account::<Safe>(self.inner.rpc(), &safe)?;
        rpc::get_account::<Whitelist>(self.inner.rpc(), &safe.whitelist).map_err(Into::into)
    }

    pub fn vault(&self, safe: &Pubkey) -> Result<TokenAccount, ClientError> {
        let safe = rpc::get_account::<Safe>(self.inner.rpc(), &safe)?;
        rpc::get_token_account::<TokenAccount>(self.inner.rpc(), &safe.vault).map_err(Into::into)
    }

    pub fn vesting(&self, addr: &Pubkey) -> Result<Vesting, ClientError> {
        rpc::get_account::<Vesting>(self.inner.rpc(), addr).map_err(Into::into)
    }
}

// Private.
impl Client {
    fn vault_authority(&self, safe_addr: Pubkey) -> Result<Pubkey, ClientError> {
        let safe = self.safe(&safe_addr)?;
        let seeds = TokenVault::signer_seeds(&safe_addr, &safe.nonce);

        Pubkey::create_program_address(&seeds, self.program()).map_err(|e| {
            anyhow::anyhow!("unable to derive vault authority: {:?}", e.to_string()).into()
        })
    }
}

impl solana_client_gen::prelude::ClientGen for Client {
    fn from_keypair_file(program_id: Pubkey, filename: &str, url: &str) -> anyhow::Result<Client> {
        Ok(Self::new(
            InnerClient::from_keypair_file(program_id, filename, url)
                .map_err(|e| anyhow::anyhow!(e.to_string()))?,
        ))
    }
    fn with_options(self, opts: RequestOptions) -> Client {
        Self::new(self.inner.with_options(opts))
    }
    fn rpc(&self) -> &RpcClient {
        self.inner.rpc()
    }
    fn payer(&self) -> &Keypair {
        self.inner.payer()
    }
    fn program(&self) -> &Pubkey {
        self.inner.program()
    }
}

pub struct InitializeRequest {
    pub mint: Pubkey,
    pub authority: Pubkey,
}

#[derive(Debug)]
pub struct InitializeResponse {
    pub tx: Signature,
    pub safe: Pubkey,
    pub vault: Pubkey,
    pub vault_authority: Pubkey,
    pub whitelist: Pubkey,
    pub nonce: u8,
}

pub struct CreateVestingRequest<'a> {
    pub depositor: Pubkey,
    pub depositor_owner: &'a Keypair,
    pub safe: Pubkey,
    pub beneficiary: Pubkey,
    pub end_slot: u64,
    pub period_count: u64,
    pub deposit_amount: u64,
}

#[derive(Debug)]
pub struct CreateVestingResponse {
    pub tx: Signature,
    pub vesting: Pubkey,
    pub mint: Pubkey,
}

pub struct WhitelistAddRequest<'a> {
    pub authority: &'a Keypair,
    pub safe: Pubkey,
    pub program: Pubkey,
}

#[derive(Debug)]
pub struct WhitelistAddResponse {
    pub tx: Signature,
}

pub struct WhitelistDeleteRequest<'a> {
    pub authority: &'a Keypair,
    pub safe: Pubkey,
    pub program: Pubkey,
}

#[derive(Debug)]
pub struct WhitelistDeleteResponse {
    pub tx: Signature,
}

pub struct WhitelistWithdrawRequest<'a> {
    pub beneficiary: &'a Keypair,
    pub vesting: Pubkey,
    pub safe: Pubkey,
    pub whitelist_program: Pubkey,
    pub relay_accounts: Vec<AccountMeta>,
    pub vault: Pubkey,
    pub whitelist_vault: Pubkey,
    pub whitelist_vault_authority: Pubkey,
    pub delegate_amount: u64,
    pub relay_data: Vec<u8>,
}

#[derive(Debug)]
pub struct WhitelistWithdrawResponse {
    pub tx: Signature,
}

pub struct WhitelistDepositRequest<'a> {
    pub beneficiary: &'a Keypair,
    pub vesting: Pubkey,
    pub safe: Pubkey,
    pub whitelist_program: Pubkey,
    pub relay_accounts: Vec<AccountMeta>,
    pub vault: Pubkey,
    pub whitelist_vault: Pubkey,
    pub whitelist_vault_authority: Pubkey,
    pub relay_data: Vec<u8>,
}

#[derive(Debug)]
pub struct WhitelistDepositResponse {
    pub tx: Signature,
}

pub struct ClaimRequest<'a> {
    pub beneficiary: &'a Keypair,
    pub safe: Pubkey,
    pub vesting: Pubkey,
    pub locked_mint: Pubkey,
    pub locked_token_account: Pubkey,
}

#[derive(Debug)]
pub struct ClaimResponse {
    pub tx: Signature,
}

pub struct RedeemRequest<'a> {
    pub beneficiary: &'a Keypair,
    pub vesting: Pubkey,
    pub token_account: Pubkey,
    pub vault: Pubkey,
    pub safe: Pubkey,
    pub locked_token_account: Pubkey,
    pub locked_mint: Pubkey,
    pub amount: u64,
}

#[derive(Debug)]
pub struct RedeemResponse {
    pub tx: Signature,
}

pub struct SetAuthorityRequest<'a> {
    pub authority: &'a Keypair,
    pub safe: Pubkey,
    pub new_authority: Pubkey,
}

#[derive(Debug)]
pub struct SetAuthorityResponse {
    pub tx: Signature,
}

pub struct MigrateRequest<'a> {
    pub authority: &'a Keypair,
    pub safe: Pubkey,
    pub new_token_account: Pubkey,
}

#[derive(Debug)]
pub struct MigrateResponse {
    pub tx: Signature,
}

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("Client error {0}")]
    InnerError(#[from] InnerClientError),
    #[error("Error invoking rpc: {0}")]
    RpcError(#[from] solana_client::client_error::ClientError),
    #[error("Any error: {0}")]
    Any(#[from] anyhow::Error),
}
