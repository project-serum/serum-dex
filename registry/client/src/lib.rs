use serum_common::client::rpc;
use serum_common::pack::*;
use serum_registry::accounts::{
    self, pending_withdrawal, vault, Entity, Member, PendingWithdrawal, Registrar,
};
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
            deactivation_timelock,
            max_stake_per_entity,
            mint,
            mega_mint,
            reward_activation_threshold,
        } = req;
        let (tx, registrar, reward_event_q, nonce, pool_vault, pool_vault_mega) =
            inner::initialize(
                &self.inner,
                &mint,
                &mega_mint,
                &registrar_authority,
                withdrawal_timelock,
                deactivation_timelock,
                reward_activation_threshold,
                max_stake_per_entity,
            )?;
        Ok(InitializeResponse {
            tx,
            registrar,
            reward_event_q,
            nonce,
            pool_vault,
            pool_vault_mega,
        })
    }

    pub fn create_entity(
        &self,
        req: CreateEntityRequest,
    ) -> Result<CreateEntityResponse, ClientError> {
        let CreateEntityRequest {
            node_leader,
            registrar,
            name,
            about,
            image_url,
            meta_entity_program_id,
        } = req;
        let (tx, entity) = inner::create_entity(
            &self.inner,
            registrar,
            node_leader,
            name,
            about,
            image_url,
            meta_entity_program_id,
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
            new_metadata,
            registrar,
        } = req;
        let accounts = [
            AccountMeta::new(entity, false),
            AccountMeta::new_readonly(leader.pubkey(), true),
            AccountMeta::new_readonly(registrar, false),
        ];
        let tx = self.inner.update_entity_with_signers(
            &[leader, self.payer()],
            &accounts,
            new_leader,
            new_metadata,
        )?;
        Ok(UpdateEntityResponse { tx })
    }

    pub fn create_member(
        &self,
        req: CreateMemberRequest,
    ) -> Result<CreateMemberResponse, ClientError> {
        let CreateMemberRequest {
            entity,
            beneficiary,
            delegate,
            registrar,
        } = req;

        let vault_authority = self.vault_authority(&registrar)?;
        let r = self.registrar(&registrar)?;

        let pool_token = Keypair::generate(&mut OsRng);
        let mega_pool_token = Keypair::generate(&mut OsRng);

        let create_pool_token_instrs = rpc::create_token_account_instructions(
            self.inner.rpc(),
            pool_token.pubkey(),
            &r.pool_mint,
            &vault_authority,
            self.inner.payer(),
        )?;
        let create_mega_pool_token_instrs = rpc::create_token_account_instructions(
            self.inner.rpc(),
            mega_pool_token.pubkey(),
            &r.pool_mint_mega,
            &vault_authority,
            self.inner.payer(),
        )?;

        let member_kp = Keypair::generate(&mut OsRng);
        let create_acc_instr = {
            let lamports = self
                .inner
                .rpc()
                .get_minimum_balance_for_rent_exemption(*accounts::member::SIZE as usize)
                .map_err(InnerClientError::RpcError)?;
            system_instruction::create_account(
                &self.inner.payer().pubkey(),
                &member_kp.pubkey(),
                lamports,
                *accounts::member::SIZE,
                self.inner.program(),
            )
        };

        let accounts = [
            AccountMeta::new_readonly(beneficiary.pubkey(), true),
            AccountMeta::new(member_kp.pubkey(), false),
            AccountMeta::new(entity, false),
            AccountMeta::new_readonly(registrar, false),
            AccountMeta::new_readonly(vault_authority, false),
            AccountMeta::new_readonly(pool_token.pubkey(), false),
            AccountMeta::new_readonly(mega_pool_token.pubkey(), false),
            AccountMeta::new_readonly(spl_token::ID, false),
            AccountMeta::new_readonly(solana_sdk::sysvar::rent::ID, false),
        ];

        let member_instr =
            serum_registry::instruction::create_member(*self.inner.program(), &accounts, delegate);

        let mut instructions = vec![];
        instructions.extend_from_slice(&create_pool_token_instrs);
        instructions.extend_from_slice(&create_mega_pool_token_instrs);
        instructions.extend_from_slice(&[create_acc_instr, member_instr]);

        let signers = vec![
            self.inner.payer(),
            &member_kp,
            beneficiary,
            &pool_token,
            &mega_pool_token,
        ];
        let (recent_hash, _fee_calc) = self.inner.rpc().get_recent_blockhash()?;

        let tx = Transaction::new_signed_with_payer(
            &instructions,
            Some(&self.inner.payer().pubkey()),
            &signers,
            recent_hash,
        );

        self.inner
            .rpc()
            .send_and_confirm_transaction_with_spinner_and_config(
                &tx,
                self.inner.options().commitment,
                self.inner.options().tx,
            )
            .map_err(ClientError::RpcError)
            .map(|sig| CreateMemberResponse {
                tx: sig,
                member: member_kp.pubkey(),
            })
    }

    pub fn deposit(&self, req: DepositRequest) -> Result<DepositResponse, ClientError> {
        let DepositRequest {
            member,
            beneficiary,
            entity,
            depositor,
            depositor_authority, // todo: remove this?
            registrar,
            amount,
        } = req;
        let vault = self.vault_for(&registrar, &depositor)?;
        let vault_acc = rpc::get_token_account::<TokenAccount>(self.inner.rpc(), &vault)?;
        let accounts = vec![
            // Whitelist relay interface,
            AccountMeta::new(depositor, false),
            AccountMeta::new(depositor_authority.pubkey(), true),
            AccountMeta::new_readonly(spl_token::ID, false),
            AccountMeta::new(vault, false),
            AccountMeta::new(vault_acc.owner, false),
            // Program specific.
            AccountMeta::new(member, false),
            AccountMeta::new_readonly(beneficiary.pubkey(), true),
            AccountMeta::new(entity, false),
            AccountMeta::new_readonly(registrar, false),
            AccountMeta::new_readonly(solana_sdk::sysvar::clock::ID, false),
        ];

        let signers = [self.payer(), beneficiary, depositor_authority];

        let tx = self
            .inner
            .deposit_with_signers(&signers, &accounts, amount)?;

        Ok(DepositResponse { tx })
    }

    pub fn withdraw(&self, req: WithdrawRequest) -> Result<WithdrawResponse, ClientError> {
        let WithdrawRequest {
            member,
            beneficiary,
            entity,
            depositor, // Owned by beneficiary.
            registrar,
            amount,
        } = req;
        let vault = self.vault_for(&registrar, &depositor)?;
        let vault_acc = rpc::get_token_account::<TokenAccount>(self.inner.rpc(), &vault)?;
        let accounts = vec![
            // Whitelist relay interface.
            AccountMeta::new(depositor, false),
            AccountMeta::new_readonly(beneficiary.pubkey(), true),
            AccountMeta::new_readonly(spl_token::ID, false),
            AccountMeta::new(vault, false),
            AccountMeta::new(vault_acc.owner, false),
            // Program specific.
            AccountMeta::new(member, false),
            AccountMeta::new_readonly(beneficiary.pubkey(), true),
            AccountMeta::new(entity, false),
            AccountMeta::new_readonly(registrar, false),
            AccountMeta::new_readonly(solana_sdk::sysvar::clock::ID, false),
        ];
        let signers = [self.payer(), beneficiary];

        let tx = self
            .inner
            .withdraw_with_signers(&signers, &accounts, amount)?;

        Ok(WithdrawResponse { tx })
    }

    pub fn stake(&self, req: StakeRequest) -> Result<StakeResponse, ClientError> {
        let StakeRequest {
            member,
            beneficiary,
            entity,
            registrar,
            pool_token_amount,
            mega, // TODO: remove.
        } = req;
        let r = self.registrar(&registrar)?;
        let (vault, pool_vault, pool_mint) = {
            if mega {
                (r.mega_vault, r.pool_vault_mega, r.pool_mint_mega)
            } else {
                (r.vault, r.pool_vault, r.pool_mint)
            }
        };
        let vault_authority = self.vault_authority(&registrar)?;
        let m_acc = self.member(&member)?;
        let spt = {
            if mega {
                m_acc.spt_mega
            } else {
                m_acc.spt
            }
        };

        let accounts = vec![
            AccountMeta::new(member, false),
            AccountMeta::new_readonly(beneficiary.pubkey(), true),
            AccountMeta::new(entity, false),
            AccountMeta::new_readonly(registrar, false),
            AccountMeta::new(vault, false),
            AccountMeta::new_readonly(vault_authority, false),
            AccountMeta::new(pool_vault, false),
            AccountMeta::new(pool_mint, false),
            AccountMeta::new(spt, false),
            AccountMeta::new_readonly(solana_sdk::sysvar::clock::ID, false),
            AccountMeta::new_readonly(spl_token::ID, false),
        ];

        let signers = [self.payer(), beneficiary];

        let tx = self
            .inner
            .stake_with_signers(&signers, &accounts, pool_token_amount)?;

        Ok(StakeResponse { tx })
    }

    pub fn start_stake_withdrawal(
        &self,
        req: StartStakeWithdrawalRequest,
    ) -> Result<StartStakeWithdrawalResponse, ClientError> {
        let StartStakeWithdrawalRequest {
            registrar,
            member,
            entity,
            beneficiary,
            spt_amount,
            mega, // TODO: remove.
        } = req;
        let pending_withdrawal = Keypair::generate(&mut OsRng);

        let r = self.registrar(&registrar)?;
        let (vault, pool_vault, pool_mint) = {
            if mega {
                (r.mega_vault, r.pool_vault_mega, r.pool_mint_mega)
            } else {
                (r.vault, r.pool_vault, r.pool_mint)
            }
        };
        let vault_authority = self.vault_authority(&registrar)?;
        let m_acc = self.member(&member)?;
        let spt = {
            if mega {
                m_acc.spt_mega
            } else {
                m_acc.spt
            }
        };

        let accs = vec![
            AccountMeta::new(pending_withdrawal.pubkey(), false),
            //
            AccountMeta::new(member, false),
            AccountMeta::new_readonly(beneficiary.pubkey(), true),
            AccountMeta::new(entity, false),
            AccountMeta::new_readonly(registrar, false),
            AccountMeta::new(vault, false),
            AccountMeta::new_readonly(vault_authority, false),
            AccountMeta::new(pool_vault, false),
            AccountMeta::new(pool_mint, false),
            AccountMeta::new(spt, false),
            AccountMeta::new_readonly(solana_sdk::sysvar::clock::ID, false),
            AccountMeta::new_readonly(spl_token::ID, false),
            AccountMeta::new_readonly(solana_sdk::sysvar::rent::ID, false),
        ];

        let instructions = {
            let create_pending_withdrawal_instr = {
                let lamports = self
                    .rpc()
                    .get_minimum_balance_for_rent_exemption(*pending_withdrawal::SIZE as usize)
                    .map_err(InnerClientError::RpcError)?;
                system_instruction::create_account(
                    &self.payer().pubkey(),
                    &pending_withdrawal.pubkey(),
                    lamports,
                    *pending_withdrawal::SIZE,
                    self.program(),
                )
            };
            let start_stake_withdrawal_instr = serum_registry::instruction::start_stake_withdrawal(
                *self.program(),
                &accs,
                spt_amount,
            );
            [
                create_pending_withdrawal_instr,
                start_stake_withdrawal_instr,
            ]
        };
        let tx = {
            let (recent_hash, _fee_calc) = self
                .rpc()
                .get_recent_blockhash()
                .map_err(|e| InnerClientError::RawError(e.to_string()))?;
            let signers = [self.payer(), beneficiary, &pending_withdrawal];
            Transaction::new_signed_with_payer(
                &instructions,
                Some(&self.payer().pubkey()),
                &signers,
                recent_hash,
            )
        };

        self.rpc()
            .send_and_confirm_transaction_with_spinner_and_config(
                &tx,
                self.inner.options().commitment,
                self.inner.options().tx,
            )
            .map_err(ClientError::RpcError)
            .map(|tx| StartStakeWithdrawalResponse {
                tx,
                pending_withdrawal: pending_withdrawal.pubkey(),
            })
    }

    pub fn end_stake_withdrawal(
        &self,
        req: EndStakeWithdrawalRequest,
    ) -> Result<EndStakeWithdrawalResponse, ClientError> {
        let EndStakeWithdrawalRequest {
            registrar,
            member,
            entity,
            beneficiary,
            pending_withdrawal,
        } = req;
        let accs = vec![
            AccountMeta::new(pending_withdrawal, false),
            AccountMeta::new(member, false),
            AccountMeta::new_readonly(beneficiary.pubkey(), true),
            AccountMeta::new(entity, false),
            AccountMeta::new_readonly(registrar, false),
            AccountMeta::new_readonly(solana_sdk::sysvar::clock::ID, false),
        ];

        let instructions = [serum_registry::instruction::end_stake_withdrawal(
            *self.program(),
            &accs,
        )];

        let tx = {
            let (recent_hash, _fee_calc) = self
                .rpc()
                .get_recent_blockhash()
                .map_err(|e| InnerClientError::RawError(e.to_string()))?;
            let signers = [self.payer(), beneficiary];
            Transaction::new_signed_with_payer(
                &instructions,
                Some(&self.payer().pubkey()),
                &signers,
                recent_hash,
            )
        };

        self.rpc()
            .send_and_confirm_transaction_with_spinner_and_config(
                &tx,
                self.inner.options().commitment,
                self.inner.options().tx,
            )
            .map_err(ClientError::RpcError)
            .map(|tx| EndStakeWithdrawalResponse { tx })
    }

    pub fn switch_entity(
        &self,
        req: SwitchEntityRequest,
    ) -> Result<SwitchEntityResponse, ClientError> {
        let SwitchEntityRequest {
            member,
            entity,
            new_entity,
            beneficiary,
            registrar,
        } = req;
        let accs = vec![
            AccountMeta::new(member, false),
            AccountMeta::new_readonly(beneficiary.pubkey(), true),
            AccountMeta::new_readonly(registrar, false),
            AccountMeta::new(entity, false),
            AccountMeta::new(new_entity, false),
            AccountMeta::new_readonly(solana_sdk::sysvar::clock::ID, false),
        ];
        let tx = self
            .inner
            .switch_entity_with_signers(&[self.payer(), beneficiary], &accs)?;
        Ok(SwitchEntityResponse { tx })
    }
}

// Account accessors.
impl Client {
    pub fn registrar(&self, registrar: &Pubkey) -> Result<Registrar, ClientError> {
        rpc::get_account::<Registrar>(self.inner.rpc(), registrar).map_err(Into::into)
    }

    pub fn entity(&self, entity: &Pubkey) -> Result<Entity, ClientError> {
        rpc::get_account_unchecked::<Entity>(self.inner.rpc(), entity).map_err(Into::into)
    }
    pub fn vault_authority(&self, registrar: &Pubkey) -> Result<Pubkey, ClientError> {
        let r = self.registrar(registrar)?;
        Pubkey::create_program_address(&vault::signer_seeds(registrar, &r.nonce), self.program())
            .map_err(|_| ClientError::Any(anyhow::anyhow!("invalid vault authority")))
    }
    pub fn member(&self, member: &Pubkey) -> Result<Member, ClientError> {
        rpc::get_account::<Member>(self.inner.rpc(), &member).map_err(Into::into)
    }
    pub fn member_seed() -> &'static str {
        inner::member_seed()
    }
    pub fn vault_for(&self, registrar: &Pubkey, depositor: &Pubkey) -> Result<Pubkey, ClientError> {
        let depositor = rpc::get_token_account::<TokenAccount>(self.inner.rpc(), depositor)?;

        let r = self.registrar(&registrar)?;

        let vault = self.current_deposit_vault(registrar)?;
        if vault.mint == depositor.mint {
            return Ok(r.vault);
        }

        let mega_vault = self.current_deposit_mega_vault(registrar)?;
        if mega_vault.mint == depositor.mint {
            return Ok(r.mega_vault);
        }
        Err(ClientError::Any(anyhow::anyhow!("invalid depositor mint")))
    }
    pub fn current_deposit_vault(&self, registrar: &Pubkey) -> Result<TokenAccount, ClientError> {
        let r = self.registrar(registrar)?;
        rpc::get_token_account::<TokenAccount>(self.inner.rpc(), &r.vault).map_err(Into::into)
    }
    pub fn current_deposit_mega_vault(
        &self,
        registrar: &Pubkey,
    ) -> Result<TokenAccount, ClientError> {
        let r = self.registrar(registrar)?;
        rpc::get_token_account::<TokenAccount>(self.inner.rpc(), &r.mega_vault).map_err(Into::into)
    }
    pub fn pool_token(&self, member: &Pubkey) -> Result<ProgramAccount<TokenAccount>, ClientError> {
        let m = self.member(member)?;
        let account = rpc::get_token_account(self.inner.rpc(), &m.spt)?;
        Ok(ProgramAccount {
            public_key: m.spt_mega,
            account,
        })
    }

    pub fn mega_pool_token(
        &self,
        member: &Pubkey,
    ) -> Result<ProgramAccount<TokenAccount>, ClientError> {
        let m = self.member(member)?;
        let account = rpc::get_token_account(self.inner.rpc(), &m.spt_mega)?;
        Ok(ProgramAccount {
            public_key: m.spt_mega,
            account,
        })
    }
    pub fn stake_pool_asset_vault(&self, registrar: &Pubkey) -> Result<TokenAccount, ClientError> {
        let r = self.registrar(registrar)?;
        rpc::get_token_account::<TokenAccount>(self.inner.rpc(), &r.pool_vault).map_err(Into::into)
    }

    pub fn stake_mega_pool_asset_vaults(
        &self,
        registrar: &Pubkey,
    ) -> Result<TokenAccount, ClientError> {
        let r = self.registrar(registrar)?;
        rpc::get_token_account::<TokenAccount>(self.inner.rpc(), &r.pool_vault_mega)
            .map_err(Into::into)
    }

    pub fn pending_withdrawal(&self, pw: &Pubkey) -> Result<PendingWithdrawal, ClientError> {
        rpc::get_account::<PendingWithdrawal>(self.inner.rpc(), pw).map_err(Into::into)
    }
}

pub struct ProgramAccount<T> {
    pub public_key: Pubkey,
    pub account: T,
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
    pub deactivation_timelock: i64,
    pub max_stake_per_entity: u64,
    pub mint: Pubkey,
    pub mega_mint: Pubkey,
    pub reward_activation_threshold: u64,
}

pub struct InitializeResponse {
    pub tx: Signature,
    pub registrar: Pubkey,
    pub reward_event_q: Pubkey,
    pub pool_vault: Pubkey,
    pub pool_vault_mega: Pubkey,
    pub nonce: u8,
}

pub struct CreateEntityRequest<'a> {
    pub node_leader: &'a Keypair,
    pub registrar: Pubkey,
    pub name: String,
    pub about: String,
    pub image_url: String,
    pub meta_entity_program_id: Pubkey,
}

pub struct CreateEntityResponse {
    pub tx: Signature,
    pub entity: Pubkey,
}

pub struct UpdateEntityRequest<'a> {
    pub entity: Pubkey,
    pub leader: &'a Keypair,
    pub new_leader: Option<Pubkey>,
    pub new_metadata: Option<Pubkey>,
    pub registrar: Pubkey,
}

pub struct UpdateEntityResponse {
    pub tx: Signature,
}

pub struct CreateMemberRequest<'a> {
    pub entity: Pubkey,
    pub delegate: Pubkey,
    pub registrar: Pubkey,
    pub beneficiary: &'a Keypair,
}

pub struct CreateMemberResponse {
    pub tx: Signature,
    pub member: Pubkey,
}

pub struct StakeRequest<'a> {
    pub member: Pubkey,
    pub beneficiary: &'a Keypair,
    pub entity: Pubkey,
    pub registrar: Pubkey,
    pub pool_token_amount: u64,
    pub mega: bool,
}

pub struct StakeResponse {
    pub tx: Signature,
}

pub struct DepositRequest<'a> {
    pub member: Pubkey,
    pub beneficiary: &'a Keypair,
    pub entity: Pubkey,
    pub depositor: Pubkey,
    pub depositor_authority: &'a Keypair,
    pub registrar: Pubkey,
    pub amount: u64,
}

pub struct DepositResponse {
    pub tx: Signature,
}

pub struct WithdrawRequest<'a> {
    pub member: Pubkey,
    pub beneficiary: &'a Keypair,
    pub entity: Pubkey,
    pub depositor: Pubkey,
    pub registrar: Pubkey,
    pub amount: u64,
}

pub struct WithdrawResponse {
    pub tx: Signature,
}

pub struct StartStakeWithdrawalRequest<'a> {
    pub registrar: Pubkey,
    pub member: Pubkey,
    pub entity: Pubkey,
    pub beneficiary: &'a Keypair,
    pub spt_amount: u64,
    pub mega: bool,
}

pub struct StartStakeWithdrawalResponse {
    pub tx: Signature,
    pub pending_withdrawal: Pubkey,
}

pub struct EndStakeWithdrawalRequest<'a> {
    pub registrar: Pubkey,
    pub member: Pubkey,
    pub entity: Pubkey,
    pub beneficiary: &'a Keypair,
    pub pending_withdrawal: Pubkey,
}

pub struct EndStakeWithdrawalResponse {
    pub tx: Signature,
}

pub struct SwitchEntityRequest<'a> {
    pub member: Pubkey,
    pub entity: Pubkey,
    pub new_entity: Pubkey,
    pub registrar: Pubkey,
    pub beneficiary: &'a Keypair,
}

pub struct SwitchEntityResponse {
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
