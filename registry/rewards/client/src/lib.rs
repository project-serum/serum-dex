use anyhow::anyhow;
use serum_common::client::rpc;
use serum_rewards::accounts::{vault, Instance};
use serum_rewards::client::{Client as InnerClient, ClientError as InnerClientError};
use serum_rewards::error::RewardsError;
use solana_client_gen::prelude::Signer;
use solana_client_gen::prelude::*;
use solana_client_gen::solana_sdk;
use solana_sdk::instruction::{AccountMeta, Instruction};
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
        let (tx, instance, nonce) = inner::initialize(
            &self.inner,
            req.registry_program_id,
            req.registrar,
            req.reward_mint,
            req.dex_program_id,
            req.authority,
        )?;
        Ok(InitializeResponse {
            tx,
            instance,
            nonce,
        })
    }

    pub fn crank_relay(&self, req: CrankRelayRequest) -> Result<CrankRelayResponse, ClientError> {
        let CrankRelayRequest {
            instance,
            token_account,
            entity,
            entity_leader,
            dex_event_q,
            mut consume_events_instr,
        } = req;
        let instance_acc = self.instance(instance)?;
        let vault_authority = Pubkey::create_program_address(
            &vault::signer_seeds(&instance, &instance_acc.nonce),
            self.program(),
        )
        .map_err(|_| ClientError::Any(anyhow!("unable to derive program address")))?;
        let signers = [self.inner.payer(), entity_leader];
        let mut accounts = vec![
            AccountMeta::new_readonly(instance, false),
            AccountMeta::new(instance_acc.vault, false),
            AccountMeta::new_readonly(vault_authority, false),
            AccountMeta::new_readonly(instance_acc.registrar, false),
            AccountMeta::new(token_account, false),
            AccountMeta::new_readonly(entity, false),
            AccountMeta::new_readonly(entity_leader.pubkey(), true),
            AccountMeta::new_readonly(spl_token::ID, false),
            AccountMeta::new_readonly(consume_events_instr.program_id, false),
            AccountMeta::new(dex_event_q, false),
        ];
        accounts.append(&mut consume_events_instr.accounts);
        let tx =
            self.inner
                .crank_relay_with_signers(&signers, &accounts, consume_events_instr.data)?;
        Ok(CrankRelayResponse { tx })
    }

    pub fn set_authority(
        &self,
        req: SetAuthorityRequest,
    ) -> Result<SetAuthorityResponse, ClientError> {
        let SetAuthorityRequest {
            instance,
            new_authority,
            authority,
        } = req;
        let i = self.instance(instance)?;
        let accounts = [
            AccountMeta::new_readonly(i.authority, false),
            AccountMeta::new(instance, false),
        ];
        let signers = [authority, self.payer()];
        let tx = self
            .inner
            .set_authority_with_signers(&signers, &accounts, new_authority)?;
        Ok(SetAuthorityResponse { tx })
    }

    pub fn migrate(&self, req: MigrateRequest) -> Result<MigrateResponse, ClientError> {
        let MigrateRequest {
            authority,
            instance,
            receiver,
        } = req;
        let i = self.instance(instance)?;
        let vault_authority = Pubkey::create_program_address(
            &vault::signer_seeds(&instance, &i.nonce),
            self.program(),
        )
        .map_err(|_| ClientError::Any(anyhow!("unable to derive program address")))?;
        let accounts = [
            AccountMeta::new_readonly(authority.pubkey(), true),
            AccountMeta::new(instance, false),
            AccountMeta::new(i.vault, false),
            AccountMeta::new_readonly(vault_authority, false),
            AccountMeta::new(receiver, false),
            AccountMeta::new_readonly(spl_token::ID, false),
        ];
        let signers = [authority, self.payer()];
        let tx = self.inner.migrate_with_signers(&signers, &accounts)?;
        Ok(MigrateResponse { tx })
    }
}

// Account accessors.
impl Client {
    pub fn instance(&self, address: Pubkey) -> Result<Instance, ClientError> {
        rpc::get_account::<Instance>(self.inner.rpc(), &address).map_err(Into::into)
    }

    pub fn vault(&self, instance: Pubkey) -> Result<TokenAccount, ClientError> {
        let instance = self.instance(instance)?;
        rpc::get_token_account::<TokenAccount>(self.inner.rpc(), &instance.vault)
            .map_err(Into::into)
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

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("Client error {0}")]
    InnerError(#[from] InnerClientError),
    #[error("Error invoking rpc: {0}")]
    RpcError(#[from] solana_client::client_error::ClientError),
    #[error("Any error: {0}")]
    Any(#[from] anyhow::Error),
    #[error("Rewards error: {0}")]
    RewardsError(#[from] RewardsError),
}

pub struct InitializeRequest {
    pub registry_program_id: Pubkey,
    pub registrar: Pubkey,
    pub reward_mint: Pubkey,
    pub dex_program_id: Pubkey,
    pub authority: Pubkey,
}

#[derive(Debug)]
pub struct InitializeResponse {
    pub tx: Signature,
    pub instance: Pubkey,
    pub nonce: u8,
}

pub struct CrankRelayRequest<'a> {
    pub instance: Pubkey,
    pub token_account: Pubkey,
    pub entity: Pubkey,
    pub entity_leader: &'a Keypair,
    pub dex_event_q: Pubkey,
    pub consume_events_instr: Instruction,
}

pub struct CrankRelayResponse {
    pub tx: Signature,
}

pub struct SetAuthorityRequest<'a> {
    pub new_authority: Pubkey,
    pub instance: Pubkey,
    pub authority: &'a Keypair,
}

pub struct SetAuthorityResponse {
    pub tx: Signature,
}

pub struct MigrateRequest<'a> {
    pub authority: &'a Keypair,
    pub instance: Pubkey,
    pub receiver: Pubkey,
}

pub struct MigrateResponse {
    pub tx: Signature,
}
