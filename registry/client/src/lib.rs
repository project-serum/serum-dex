use serum_common::client::rpc;
use serum_registry::accounts::{Entity, Member, Registrar, StakeKind};
use serum_registry::client::{Client as InnerClient, ClientError as InnerClientError};
use solana_client_gen::prelude::*;
use solana_client_gen::solana_sdk::instruction::AccountMeta;
use solana_client_gen::solana_sdk::pubkey::Pubkey;
use solana_client_gen::solana_sdk::signature::Signature;
use solana_client_gen::solana_sdk::signature::{Keypair, Signer};
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
        let InitializeRequest {
            registrar_authority,
            withdrawal_timelock,
            deactivation_timelock_premium,
            mint,
            mega_mint,
            reward_activation_threshold,
        } = req;
        let (tx, registrar, nonce) = inner::initialize(
            &self.inner,
            &mint,
            &mega_mint,
            &registrar_authority,
            withdrawal_timelock,
            deactivation_timelock_premium,
            reward_activation_threshold,
        )?;
        Ok(InitializeResponse {
            tx,
            registrar,
            nonce,
        })
    }

    pub fn register_capability(
        &self,
        req: RegisterCapabilityRequest,
    ) -> Result<RegisterCapabilityResponse, ClientError> {
        let RegisterCapabilityRequest {
            registrar,
            registrar_authority,
            capability_id,
            capability_fee_bps,
        } = req;
        let accounts = [
            AccountMeta::new_readonly(registrar_authority.pubkey(), true),
            AccountMeta::new(registrar, false),
        ];
        let signers = [registrar_authority, self.payer()];
        let tx = self.inner.register_capability_with_signers(
            &signers,
            &accounts,
            capability_id,
            capability_fee_bps,
        )?;
        Ok(RegisterCapabilityResponse { tx })
    }

    pub fn create_entity(
        &self,
        req: CreateEntityRequest,
    ) -> Result<CreateEntityResponse, ClientError> {
        let CreateEntityRequest {
            node_leader,
            capabilities,
            stake_kind,
            registrar,
        } = req;
        let (tx, entity) = inner::create_entity_derived(
            &self.inner,
            registrar,
            node_leader,
            capabilities,
            stake_kind,
        )?;
        Ok(CreateEntityResponse { tx, entity })
    }

    pub fn update_entity(
        &self,
        req: UpdateEntityRequest,
    ) -> Result<UpdateEntityResponse, ClientError> {
        let UpdateEntityRequest {
            entity,
            leader,
            new_leader,
            new_capabilities,
        } = req;
        let accounts = [
            AccountMeta::new(entity, false),
            AccountMeta::new_readonly(leader.pubkey(), true),
        ];
        let tx = self.inner.update_entity_with_signers(
            &[leader, self.payer()],
            &accounts,
            new_leader,
            new_capabilities,
        )?;
        Ok(UpdateEntityResponse { tx })
    }

    pub fn join_entity(&self, req: JoinEntityRequest) -> Result<JoinEntityResponse, ClientError> {
        let JoinEntityRequest {
            entity,
            beneficiary,
            delegate,
            registrar,
            watchtower,
            watchtower_dest,
        } = req;
        let (tx, member) = inner::join_entity_derived(
            &self.inner,
            registrar,
            entity,
            beneficiary,
            delegate,
            watchtower,
            watchtower_dest,
        )?;
        Ok(JoinEntityResponse { tx, member })
    }

    pub fn stake(&self, req: StakeRequest) -> Result<StakeResponse, ClientError> {
        Ok(StakeResponse {})
    }

    pub fn stake_intent(
        &self,
        req: StakeIntentRequest,
    ) -> Result<StakeIntentResponse, ClientError> {
        let StakeIntentRequest {
            member,
            beneficiary,
            entity,
            depositor,
            depositor_authority,
            mega,
            registrar,
            amount,
        } = req;
        let vault = self.registrar(&registrar)?.vault;
        let delegate = false;
        let accounts = [
            AccountMeta::new_readonly(solana_sdk::sysvar::rent::ID, false), // Dummy.
            AccountMeta::new(depositor, false),
            AccountMeta::new(vault, false),
            AccountMeta::new(depositor_authority.pubkey(), true),
            AccountMeta::new_readonly(spl_token::ID, false),
            AccountMeta::new(member, false),
            AccountMeta::new_readonly(beneficiary.pubkey(), true),
            AccountMeta::new(entity, false),
            AccountMeta::new_readonly(registrar, false),
            AccountMeta::new_readonly(solana_sdk::sysvar::clock::ID, false),
        ];
        let signers = [self.payer(), beneficiary, depositor_authority];

        let tx = self
            .inner
            .stake_intent_with_signers(&signers, &accounts, amount, mega, delegate)?;

        Ok(StakeIntentResponse { tx })
    }

    pub fn stake_intent_withdrawal(
        &self,
        req: StakeIntentWithdrawalRequest,
    ) -> Result<StakeIntentWithdrawalResponse, ClientError> {
        let StakeIntentWithdrawalRequest {
            member,
            beneficiary,
            entity,
            depositor,
            mega,
            registrar,
            amount,
        } = req;
        let r = self.registrar(&registrar)?;
        let vault = r.vault;
        let vault_acc = rpc::get_token_account::<TokenAccount>(self.inner.rpc(), &r.vault)?;
        let delegate = false;
        let accounts = [
            AccountMeta::new_readonly(solana_sdk::sysvar::rent::ID, false), // Dummy.
            AccountMeta::new(depositor, false),
            AccountMeta::new(vault, false),
            AccountMeta::new(vault_acc.owner, false),
            AccountMeta::new_readonly(spl_token::ID, false),
            AccountMeta::new(member, false),
            AccountMeta::new_readonly(beneficiary.pubkey(), true),
            AccountMeta::new(entity, false),
            AccountMeta::new_readonly(registrar, false),
            AccountMeta::new_readonly(solana_sdk::sysvar::clock::ID, false),
        ];
        let signers = [self.payer(), beneficiary];

        let tx = self
            .inner
            .stake_intent_withdrawal_with_signers(&signers, &accounts, amount, mega, delegate)?;

        Ok(StakeIntentWithdrawalResponse { tx })
    }

    pub fn start_stake_withdrawal(
        &self,
        req: StartStakeWithdrawalRequest,
    ) -> Result<StartStakeWithdrawalResponse, ClientError> {
        Ok(StartStakeWithdrawalResponse {})
    }

    pub fn end_stake_withdrawal(
        &self,
        req: EndStakeWithdrawalRequest,
    ) -> Result<EndStakeWithdrawalResponse, ClientError> {
        Ok(EndStakeWithdrawalResponse {})
    }

    pub fn donate(&self, req: DonateRequest) -> Result<DonateResponse, ClientError> {
        Ok(DonateResponse {})
    }
}

// Account accessors.
impl Client {
    pub fn registrar(&self, registrar: &Pubkey) -> Result<Registrar, ClientError> {
        rpc::get_account::<Registrar>(self.inner.rpc(), registrar).map_err(Into::into)
    }
    pub fn entity(&self, entity: &Pubkey) -> Result<Entity, ClientError> {
        rpc::get_account::<Entity>(self.inner.rpc(), entity).map_err(Into::into)
    }
    pub fn member(&self, member: &Pubkey) -> Result<Member, ClientError> {
        rpc::get_account::<Member>(self.inner.rpc(), &member).map_err(Into::into)
    }
    pub fn member_seed() -> &'static str {
        inner::member_seed()
    }
    pub fn stake_intent_vault(&self, registrar: &Pubkey) -> Result<TokenAccount, ClientError> {
        let r = self.registrar(registrar)?;
        rpc::get_token_account::<TokenAccount>(self.inner.rpc(), &r.vault).map_err(Into::into)
    }
    pub fn stake_intent_mega_vault(&self, registrar: &Pubkey) -> Result<TokenAccount, ClientError> {
        let r = self.registrar(registrar)?;
        rpc::get_token_account::<TokenAccount>(self.inner.rpc(), &r.mega_vault).map_err(Into::into)
    }
}

impl ClientGen for Client {
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
    pub registrar_authority: Pubkey,
    pub withdrawal_timelock: i64,
    pub deactivation_timelock_premium: i64,
    pub mint: Pubkey,
    pub mega_mint: Pubkey,
    pub reward_activation_threshold: u64,
}

pub struct InitializeResponse {
    pub tx: Signature,
    pub registrar: Pubkey,
    pub nonce: u8,
}

pub struct RegisterCapabilityRequest<'a> {
    pub registrar: Pubkey,
    pub registrar_authority: &'a Keypair,
    pub capability_id: u8,
    pub capability_fee_bps: u32,
}

pub struct RegisterCapabilityResponse {
    pub tx: Signature,
}

pub struct CreateEntityRequest<'a> {
    pub node_leader: &'a Keypair,
    pub capabilities: u32,
    pub stake_kind: StakeKind,
    pub registrar: Pubkey,
}

pub struct CreateEntityResponse {
    pub tx: Signature,
    pub entity: Pubkey,
}

pub struct UpdateEntityRequest<'a> {
    pub entity: Pubkey,
    pub leader: &'a Keypair,
    pub new_capabilities: u32,
    pub new_leader: Pubkey,
}

pub struct UpdateEntityResponse {
    pub tx: Signature,
}

pub struct JoinEntityRequest {
    pub entity: Pubkey,
    pub delegate: Pubkey,
    pub registrar: Pubkey,
    // TODO: take in keypair instead?
    pub beneficiary: Pubkey,
    pub watchtower: Pubkey,
    pub watchtower_dest: Pubkey,
}

pub struct JoinEntityResponse {
    pub tx: Signature,
    pub member: Pubkey,
}

pub struct StakeRequest {}

pub struct StakeResponse {}

pub struct StakeIntentRequest<'a> {
    pub member: Pubkey,
    pub beneficiary: &'a Keypair,
    pub entity: Pubkey,
    pub depositor: Pubkey,
    pub depositor_authority: &'a Keypair,
    pub mega: bool,
    pub registrar: Pubkey,
    pub amount: u64,
}

pub struct StakeIntentResponse {
    pub tx: Signature,
}

pub struct StakeIntentWithdrawalRequest<'a> {
    pub member: Pubkey,
    pub beneficiary: &'a Keypair,
    pub entity: Pubkey,
    pub depositor: Pubkey,
    pub mega: bool,
    pub registrar: Pubkey,
    pub amount: u64,
}

pub struct StakeIntentWithdrawalResponse {
    pub tx: Signature,
}

pub struct DelegateStakeRequest {}

pub struct DelegateStakeResponse {}

pub struct DelegateStakeIntentRequest {}

pub struct DelegateStakeIntentResponse {}

pub struct StartStakeWithdrawalRequest {}

pub struct StartStakeWithdrawalResponse {}

pub struct EndStakeWithdrawalRequest {}

pub struct EndStakeWithdrawalResponse {}

pub struct DonateRequest {}

pub struct DonateResponse {}

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("Client error {0}")]
    InnerError(#[from] InnerClientError),
    #[error("Error invoking rpc: {0}")]
    RpcError(#[from] solana_client::client_error::ClientError),
    #[error("Any error: {0}")]
    Any(#[from] anyhow::Error),
}
