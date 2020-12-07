use anyhow::anyhow;
use serum_common::client::rpc;
use serum_common::pack::*;
use serum_lockup::accounts::{vault, Safe, Vesting, Whitelist, WhitelistEntry};
use serum_lockup::client::{Client as InnerClient, ClientError as InnerClientError};
use serum_lockup::error::LockupError;
use serum_registry::accounts::vault as registry_vault;
use serum_registry::client::Client as RegistryClientInner;
use serum_registry::instruction::RegistryInstruction;
use serum_registry_client::{Client as RegistryClient, ClientError as RegistryClientError};
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
        inner::create_all_accounts_and_initialize(&self.inner, &req.authority).map_err(Into::into)
    }

    pub fn create_vesting(
        &self,
        req: CreateVestingRequest,
    ) -> Result<CreateVestingResponse, ClientError> {
        inner::create_vesting_account(
            &self.inner,
            &req.depositor,
            req.depositor_owner,
            &req.safe,
            &req.beneficiary,
            req.end_ts,
            req.period_count,
            req.deposit_amount,
        )
        .map_err(Into::into)
        .map(|r| CreateVestingResponse {
            tx: r.0,
            vesting: r.1.pubkey(),
        })
    }

    pub fn whitelist_add(
        &self,
        req: WhitelistAddRequest,
    ) -> Result<WhitelistAddResponse, ClientError> {
        let WhitelistAddRequest {
            authority,
            safe,
            entry,
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
            .whitelist_add_with_signers(&signers, &accounts, entry)?;
        Ok(WhitelistAddResponse { tx })
    }

    pub fn whitelist_delete(
        &self,
        req: WhitelistDeleteRequest,
    ) -> Result<WhitelistDeleteResponse, ClientError> {
        let WhitelistDeleteRequest {
            authority,
            safe,
            entry,
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
            .whitelist_delete_with_signers(&signers, &accounts, entry)?;
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
            whitelist_program_vault,
            whitelist_program_vault_authority,
            delegate_amount,
            relay_data,
            mut relay_signers,
        } = req;
        let s = self.safe(&safe)?;
        let v = self.vesting(&vesting)?;
        let vault = v.vault;
        let whitelist = s.whitelist;
        let mut accounts = vec![
            AccountMeta::new_readonly(beneficiary.pubkey(), true),
            AccountMeta::new(vesting, false),
            AccountMeta::new_readonly(safe, false),
            AccountMeta::new_readonly(whitelist, false),
            AccountMeta::new_readonly(whitelist_program, false),
            // Below are relay accounts.
            AccountMeta::new(vault, false),
            AccountMeta::new_readonly(
                self.vault_authority(safe, vesting, beneficiary.pubkey())?,
                false,
            ),
            AccountMeta::new_readonly(spl_token::ID, false),
            AccountMeta::new(whitelist_program_vault, false),
            AccountMeta::new_readonly(whitelist_program_vault_authority, false),
        ];
        accounts.append(&mut relay_accounts);

        let mut signers = vec![self.payer(), &beneficiary];
        signers.append(&mut relay_signers);

        let tx = self.inner.whitelist_withdraw_with_signers(
            &signers,
            &accounts,
            delegate_amount,
            relay_data,
        )?;

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
            whitelist_program_vault,
            whitelist_program_vault_authority,
            relay_data,
            mut relay_accounts,
            mut relay_signers,
        } = req;
        let v = self.vesting(&vesting)?;
        let s = self.safe(&safe)?;
        let vault = v.vault;
        let whitelist = s.whitelist;
        let mut accounts = vec![
            AccountMeta::new_readonly(beneficiary.pubkey(), true),
            AccountMeta::new(vesting, false),
            AccountMeta::new_readonly(safe, false),
            AccountMeta::new_readonly(whitelist, false),
            AccountMeta::new_readonly(whitelist_program, false),
            // Below are relay accounts.
            //
            // Whitelist relay interface.
            AccountMeta::new(vault, false),
            AccountMeta::new_readonly(
                self.vault_authority(safe, vesting, beneficiary.pubkey())?,
                false,
            ),
            AccountMeta::new_readonly(spl_token::ID, false),
            AccountMeta::new(whitelist_program_vault, false),
            AccountMeta::new_readonly(whitelist_program_vault_authority, false),
        ];
        // Program specific relay.
        accounts.append(&mut relay_accounts);

        let mut signers = vec![self.payer(), &beneficiary];
        signers.append(&mut relay_signers);

        let tx = self
            .inner
            .whitelist_deposit_with_signers(&signers, &accounts, relay_data)?;

        Ok(WhitelistDepositResponse { tx })
    }

    pub fn withdraw(&self, req: WithdrawRequest) -> Result<WithdrawResponse, ClientError> {
        let WithdrawRequest {
            beneficiary,
            vesting,
            token_account,
            safe,
            amount,
        } = req;
        let v = self.vesting(&vesting)?;
        let vault = v.vault;
        let accounts = [
            AccountMeta::new_readonly(beneficiary.pubkey(), true),
            AccountMeta::new(vesting, false),
            AccountMeta::new(token_account, false),
            AccountMeta::new(vault, false),
            AccountMeta::new_readonly(
                self.vault_authority(safe, vesting, beneficiary.pubkey())?,
                false,
            ),
            AccountMeta::new_readonly(safe, false),
            AccountMeta::new_readonly(spl_token::ID, false),
            AccountMeta::new_readonly(solana_sdk::sysvar::clock::ID, false),
        ];
        let signers = [self.payer(), &beneficiary];
        let tx = self
            .inner
            .withdraw_with_signers(&signers, &accounts, amount)?;
        Ok(WithdrawResponse { tx })
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

    pub fn registry_deposit(
        &self,
        req: RegistryDepositRequest,
    ) -> Result<RegistryDepositResponse, ClientError> {
        let RegistryDepositRequest {
            amount,
            registry_pid,
            registrar,
            member,
            entity,
            beneficiary,
            stake_beneficiary,
            vesting,
            safe,
        } = req;
        let relay_data = {
            let instr = RegistryInstruction::Deposit { amount };
            let mut relay_data = vec![0; instr.size().unwrap() as usize];
            RegistryInstruction::pack(instr, &mut relay_data).unwrap();
            relay_data
        };

        let r_client = RegistryClient::new(RegistryClientInner::new(
            registry_pid,
            Keypair::from_bytes(&self.payer().to_bytes()).expect("invalid payer"),
            self.inner.url(),
            Some(self.inner.options().clone()),
        ));
        let r = r_client.registrar(&registrar)?;
        let whitelist_program_vault_authority = Pubkey::create_program_address(
            &registry_vault::signer_seeds(&registrar, &r.nonce),
            &registry_pid,
        )
        .map_err(|_| anyhow!("unable to create vault authority"))?;
        let v = self.vesting(&vesting)?;
        let whitelist_program_vault = r_client.vault_for(&member, &v.vault, true)?;

        let relay_accounts = vec![
            AccountMeta::new(member, false),
            AccountMeta::new_readonly(stake_beneficiary.pubkey(), true),
            AccountMeta::new(entity, false),
            AccountMeta::new_readonly(registrar, false),
        ];

        let resp = self.whitelist_withdraw(WhitelistWithdrawRequest {
            beneficiary,
            vesting,
            safe,
            whitelist_program: registry_pid,
            relay_accounts,
            whitelist_program_vault,
            whitelist_program_vault_authority,
            delegate_amount: amount,
            relay_data,
            relay_signers: vec![&stake_beneficiary],
        })?;
        Ok(RegistryDepositResponse { tx: resp.tx })
    }

    pub fn registry_withdraw(
        &self,
        req: RegistryWithdrawRequest,
    ) -> Result<RegistryWithdrawResponse, ClientError> {
        let RegistryWithdrawRequest {
            amount,
            registry_pid,
            registrar,
            member,
            entity,
            beneficiary,
            stake_beneficiary,
            vesting,
            safe,
        } = req;
        let relay_data = {
            let instr = RegistryInstruction::Withdraw { amount };
            let mut relay_data = vec![0; instr.size().unwrap() as usize];
            RegistryInstruction::pack(instr, &mut relay_data).unwrap();
            relay_data
        };

        let r_client = RegistryClient::new(RegistryClientInner::new(
            registry_pid,
            Keypair::from_bytes(&self.payer().to_bytes()).expect("invalid payer"),
            self.inner.url(),
            Some(self.inner.options().clone()),
        ));
        let r = r_client.registrar(&registrar)?;
        let whitelist_program_vault_authority = Pubkey::create_program_address(
            &registry_vault::signer_seeds(&registrar, &r.nonce),
            &registry_pid,
        )
        .map_err(|_| anyhow!("unable to create vault authority"))?;
        let v = self.vesting(&vesting)?;
        let whitelist_program_vault = r_client.vault_for(&member, &v.vault, true)?;

        let relay_accounts = vec![
            AccountMeta::new(member, false),
            AccountMeta::new_readonly(stake_beneficiary.pubkey(), true),
            AccountMeta::new(entity, false),
            AccountMeta::new_readonly(registrar, false),
        ];

        let resp = self.whitelist_deposit(WhitelistDepositRequest {
            beneficiary,
            vesting,
            safe,
            whitelist_program: registry_pid,
            whitelist_program_vault,
            whitelist_program_vault_authority,
            relay_data,
            relay_accounts,
            relay_signers: vec![&stake_beneficiary],
        })?;

        Ok(RegistryWithdrawResponse { tx: resp.tx })
    }
}

// Account accessors.
impl Client {
    pub fn safe(&self, address: &Pubkey) -> Result<Safe, ClientError> {
        rpc::get_account::<Safe>(self.inner.rpc(), address).map_err(Into::into)
    }

    // with_whitelist takes in a closure rather than returning a Whitelist
    // struct, because the Whitelist struct provides a view into the backing
    // storage array (rather than unpacking all bytes into an owned object)
    // -- due to the fact that the Whitelist is larger than the BPF stack can
    // handle. As a result, the lifetime of the Whitelist struct is tied to
    // the lifetime of the backing data array, which is defined this function.
    // In other words, the Whitelist struct can't outlive this function call;
    // hence the closure.
    pub fn with_whitelist(
        &self,
        safe: &Pubkey,
        f: impl FnOnce(Whitelist),
    ) -> Result<(), ClientError> {
        let safe = rpc::get_account::<Safe>(self.inner.rpc(), &safe)?;
        let account = self
            .inner
            .rpc()
            .get_account_with_commitment(&safe.whitelist, CommitmentConfig::recent())?
            .value
            .map_or(Err(anyhow!("Account not found")), Ok)?;
        let pk_acc = &mut (safe.whitelist, account);
        let wl = Whitelist::new(pk_acc.into())?;

        f(wl);

        Ok(())
    }

    pub fn vault_for(&self, vesting: &Pubkey) -> Result<TokenAccount, ClientError> {
        let v = rpc::get_account::<Vesting>(self.inner.rpc(), &vesting)?;
        rpc::get_token_account::<TokenAccount>(self.inner.rpc(), &v.vault).map_err(Into::into)
    }

    pub fn vesting(&self, addr: &Pubkey) -> Result<Vesting, ClientError> {
        rpc::get_account_unchecked::<Vesting>(self.inner.rpc(), addr).map_err(Into::into)
    }

    pub fn vault_authority(
        &self,
        safe_addr: Pubkey,
        vesting_addr: Pubkey,
        beneficiary: Pubkey,
    ) -> Result<Pubkey, ClientError> {
        let v = self.vesting(&vesting_addr)?;
        let seeds = vault::signer_seeds(&safe_addr, &beneficiary, &v.nonce);

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
    pub authority: Pubkey,
}

#[derive(Debug)]
pub struct InitializeResponse {
    pub tx: Signature,
    pub safe: Pubkey,
    pub whitelist: Pubkey,
}

pub struct CreateVestingRequest<'a> {
    pub depositor: Pubkey,
    pub depositor_owner: &'a Keypair,
    pub safe: Pubkey,
    pub beneficiary: Pubkey,
    pub end_ts: i64,
    pub period_count: u64,
    pub deposit_amount: u64,
}

#[derive(Debug)]
pub struct CreateVestingResponse {
    pub tx: Signature,
    pub vesting: Pubkey,
}

pub struct WhitelistAddRequest<'a> {
    pub authority: &'a Keypair,
    pub safe: Pubkey,
    pub entry: WhitelistEntry,
}

#[derive(Debug)]
pub struct WhitelistAddResponse {
    pub tx: Signature,
}

pub struct WhitelistDeleteRequest<'a> {
    pub authority: &'a Keypair,
    pub safe: Pubkey,
    pub entry: WhitelistEntry,
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
    pub whitelist_program_vault: Pubkey,
    pub whitelist_program_vault_authority: Pubkey,
    pub delegate_amount: u64,
    pub relay_accounts: Vec<AccountMeta>,
    pub relay_data: Vec<u8>,
    pub relay_signers: Vec<&'a Keypair>,
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
    pub whitelist_program_vault: Pubkey,
    pub whitelist_program_vault_authority: Pubkey,
    pub relay_accounts: Vec<AccountMeta>,
    pub relay_data: Vec<u8>,
    pub relay_signers: Vec<&'a Keypair>,
}

#[derive(Debug)]
pub struct WhitelistDepositResponse {
    pub tx: Signature,
}

pub struct WithdrawRequest<'a> {
    pub beneficiary: &'a Keypair,
    pub vesting: Pubkey,
    pub token_account: Pubkey,
    pub safe: Pubkey,
    pub amount: u64,
}

#[derive(Debug)]
pub struct WithdrawResponse {
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

pub struct RegistryDepositRequest<'a> {
    pub amount: u64,
    pub registry_pid: Pubkey,
    pub registrar: Pubkey,
    pub member: Pubkey,
    pub entity: Pubkey,
    pub vesting: Pubkey,
    pub safe: Pubkey,
    pub beneficiary: &'a Keypair,
    pub stake_beneficiary: &'a Keypair,
}

pub struct RegistryDepositResponse {
    pub tx: Signature,
}

pub struct RegistryWithdrawRequest<'a> {
    pub amount: u64,
    pub registry_pid: Pubkey,
    pub registrar: Pubkey,
    pub member: Pubkey,
    pub entity: Pubkey,
    pub vesting: Pubkey,
    pub safe: Pubkey,
    pub beneficiary: &'a Keypair,
    pub stake_beneficiary: &'a Keypair,
}

pub struct RegistryWithdrawResponse {
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
    #[error("Lockup error: {0}")]
    LockupError(#[from] LockupError),
    #[error("Registry client error: {0}")]
    RegistryClientError(#[from] RegistryClientError),
}
