use serum_common::client::rpc;
use serum_common::pack::*;
use serum_meta_entity::client::Client as MetaEntityClient;
use serum_registry::accounts::reward_queue::{RewardEventQueue, Ring};
use serum_registry::accounts::{
    self, pending_withdrawal, vault, BalanceSandbox, Entity, LockedRewardVendor, Member,
    PendingWithdrawal, Registrar, UnlockedRewardVendor,
};
use serum_registry::client::{Client as InnerClient, ClientError as InnerClientError};
use solana_client_gen::prelude::*;
use solana_client_gen::solana_sdk::instruction::AccountMeta;
use solana_client_gen::solana_sdk::pubkey::Pubkey;
use solana_client_gen::solana_sdk::signature::Signature;
use solana_client_gen::solana_sdk::signature::{Keypair, Signer};
use solana_client_gen::solana_sdk::sysvar;
use spl_token::state::Account as TokenAccount;
use std::convert::Into;
use thiserror::Error;

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
            stake_rate,
            stake_rate_mega,
        } = req;

        let reward_event_q = Keypair::generate(&mut OsRng);
        let registrar_kp = Keypair::generate(&mut OsRng);
        let (registrar_vault_authority, nonce) =
            Pubkey::find_program_address(&[registrar_kp.pubkey().as_ref()], self.inner.program());

        let decimals = 0;
        let pool_mint = rpc::new_mint(
            self.rpc(),
            self.inner.payer(),
            &registrar_vault_authority,
            decimals,
        )
        .map_err(|e| InnerClientError::RawError(e.to_string()))?
        .0
        .pubkey();

        let mega_pool_mint = rpc::new_mint(
            self.rpc(),
            self.inner.payer(),
            &registrar_vault_authority,
            decimals,
        )
        .map_err(|e| InnerClientError::RawError(e.to_string()))?
        .0
        .pubkey();

        // Build the instructions.
        let ixs = {
            let create_registrar_acc_instr = {
                let lamports = self
                    .inner
                    .rpc()
                    .get_minimum_balance_for_rent_exemption(*accounts::registrar::SIZE as usize)
                    .map_err(InnerClientError::RpcError)?;
                system_instruction::create_account(
                    &self.inner.payer().pubkey(),
                    &registrar_kp.pubkey(),
                    lamports,
                    *accounts::registrar::SIZE,
                    self.inner.program(),
                )
            };
            let create_reward_event_q_instr = {
                let data_size = RewardEventQueue::buffer_size(RewardEventQueue::RING_CAPACITY);
                let lamports = self
                    .inner
                    .rpc()
                    .get_minimum_balance_for_rent_exemption(data_size)?;
                solana_sdk::system_instruction::create_account(
                    &self.inner.payer().pubkey(),
                    &reward_event_q.pubkey(),
                    lamports,
                    data_size as u64,
                    self.inner.program(),
                )
            };

            let initialize_registrar_instr = {
                let accounts = [
                    AccountMeta::new(registrar_kp.pubkey(), false),
                    AccountMeta::new_readonly(pool_mint, false),
                    AccountMeta::new_readonly(mega_pool_mint, false),
                    AccountMeta::new(reward_event_q.pubkey(), false),
                    AccountMeta::new_readonly(solana_sdk::sysvar::rent::id(), false),
                ];
                serum_registry::instruction::initialize(
                    *self.inner.program(),
                    &accounts,
                    registrar_authority,
                    mint,
                    mega_mint,
                    nonce,
                    withdrawal_timelock,
                    deactivation_timelock,
                    max_stake_per_entity,
                    stake_rate,
                    stake_rate_mega,
                )
            };

            vec![
                create_reward_event_q_instr,
                create_registrar_acc_instr,
                initialize_registrar_instr,
            ]
        };

        let (recent_hash, _fee_calc) = self
            .inner
            .rpc()
            .get_recent_blockhash()
            .map_err(|e| InnerClientError::RawError(e.to_string()))?;
        let tx = Transaction::new_signed_with_payer(
            &ixs,
            Some(&self.inner.payer().pubkey()),
            &vec![self.inner.payer(), &reward_event_q, &registrar_kp],
            recent_hash,
        );
        let sig = self
            .inner
            .rpc()
            .send_and_confirm_transaction_with_spinner_and_config(
                &tx,
                self.inner.options().commitment,
                self.inner.options().tx,
            )
            .map_err(InnerClientError::RpcError)?;

        Ok(InitializeResponse {
            tx: sig,
            registrar: registrar_kp.pubkey(),
            reward_event_q: reward_event_q.pubkey(),
            nonce,
        })
    }
    pub fn create_entity(
        &self,
        req: CreateEntityRequest,
    ) -> Result<CreateEntityResponse, ClientError> {
        let CreateEntityRequest {
            node_leader,
            registrar,
            metadata,
        } = req;
        let entity_kp = Keypair::generate(&mut OsRng);
        let create_acc_instr = {
            let lamports = self
                .inner
                .rpc()
                .get_minimum_balance_for_rent_exemption(*accounts::entity::SIZE as usize)
                .map_err(InnerClientError::RpcError)?;
            system_instruction::create_account(
                &self.inner.payer().pubkey(),
                &entity_kp.pubkey(),
                lamports,
                *accounts::entity::SIZE,
                self.inner.program(),
            )
        };

        let accounts = [
            AccountMeta::new(entity_kp.pubkey(), false),
            AccountMeta::new_readonly(node_leader.pubkey(), true),
            AccountMeta::new_readonly(registrar, false),
            AccountMeta::new_readonly(solana_sdk::sysvar::rent::ID, false),
        ];

        let metadata_kp = Keypair::generate(&mut OsRng);

        let create_entity_instr = serum_registry::instruction::create_entity(
            *self.inner.program(),
            &accounts,
            metadata_kp.pubkey(),
        );
        let mut instructions = vec![];
        let mut signers = vec![self.payer(), node_leader, &entity_kp];
        if let Some(metadata) = metadata {
            let EntityMetadata {
                name,
                about,
                image_url,
                meta_entity_program_id,
            } = metadata;
            let create_md_instrs = create_metadata_instructions(
                self.rpc(),
                &self.inner.payer().pubkey(),
                &metadata_kp,
                &meta_entity_program_id,
                &entity_kp.pubkey(),
                name,
                about,
                image_url,
            );
            instructions.extend_from_slice(&create_md_instrs);
            signers.extend_from_slice(&[&metadata_kp])
        }

        instructions.extend_from_slice(&[create_acc_instr, create_entity_instr]);

        let (recent_hash, _fee_calc) = self.rpc().get_recent_blockhash()?;

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
            .map(|sig| CreateEntityResponse {
                tx: sig,
                entity: entity_kp.pubkey(),
            })
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

    pub fn update_entity_metadata(
        &self,
        req: UpdateEntityMetadataRequest,
    ) -> Result<UpdateEntityMetadataResponse, ClientError> {
        let UpdateEntityMetadataRequest {
            name,
            about,
            image_url,
            meta_entity_pid,
            entity,
        } = req;

        let entity = self.entity(&entity)?;

        let accounts = [
            AccountMeta::new(entity.metadata, false),
            AccountMeta::new_readonly(self.payer().pubkey(), true),
        ];

        let client = MetaEntityClient::new(
            meta_entity_pid,
            Keypair::from_bytes(&self.payer().to_bytes()).expect("invalid payer"),
            self.inner.url(),
            Some(self.inner.options().clone()),
        );
        client
            .update(&accounts, name, about, image_url, None)
            .map(|tx| UpdateEntityMetadataResponse { tx })
            .map_err(|err| ClientError::Any(anyhow::anyhow!("{}", err.to_string())))
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

        let BalanceAccounts {
            spt,
            spt_mega,
            vault,
            vault_mega,
            vault_stake,
            vault_stake_mega,
            vault_pw,
            vault_pw_mega,
            ..
        } = self.create_member_accounts(&r, vault_authority)?;

        let BalanceAccounts {
            spt: locked_spt,
            spt_mega: locked_spt_mega,
            vault: locked_vault,
            vault_mega: locked_vault_mega,
            vault_stake: locked_vault_stake,
            vault_stake_mega: locked_vault_stake_mega,
            vault_pw: locked_vault_pw,
            vault_pw_mega: locked_vault_pw_mega,
            ..
        } = self.create_member_accounts(&r, vault_authority)?;

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
            AccountMeta::new_readonly(spl_token::ID, false),
            AccountMeta::new_readonly(solana_sdk::sysvar::rent::ID, false),
            // Main.
            AccountMeta::new_readonly(beneficiary.pubkey(), false),
            AccountMeta::new(spt.pubkey(), false),
            AccountMeta::new(spt_mega.pubkey(), false),
            AccountMeta::new_readonly(vault.pubkey(), false),
            AccountMeta::new_readonly(vault_mega.pubkey(), false),
            AccountMeta::new_readonly(vault_stake.pubkey(), false),
            AccountMeta::new_readonly(vault_stake_mega.pubkey(), false),
            AccountMeta::new_readonly(vault_pw.pubkey(), false),
            AccountMeta::new_readonly(vault_pw_mega.pubkey(), false),
            // Locked.
            AccountMeta::new_readonly(delegate, false),
            AccountMeta::new(locked_spt.pubkey(), false),
            AccountMeta::new(locked_spt_mega.pubkey(), false),
            AccountMeta::new_readonly(locked_vault.pubkey(), false),
            AccountMeta::new_readonly(locked_vault_mega.pubkey(), false),
            AccountMeta::new_readonly(locked_vault_stake.pubkey(), false),
            AccountMeta::new_readonly(locked_vault_stake_mega.pubkey(), false),
            AccountMeta::new_readonly(locked_vault_pw.pubkey(), false),
            AccountMeta::new_readonly(locked_vault_pw_mega.pubkey(), false),
        ];

        let member_instr =
            serum_registry::instruction::create_member(*self.inner.program(), &accounts);

        let mut instructions = vec![];
        instructions.extend_from_slice(&[create_acc_instr, member_instr]);

        let signers = vec![self.inner.payer(), &member_kp, beneficiary];
        let (recent_hash, _fee_calc) = self.rpc().get_recent_blockhash()?;

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

    fn create_member_accounts(
        &self,
        r: &Registrar,
        vault_authority: Pubkey,
    ) -> Result<BalanceAccounts, ClientError> {
        let spt = Keypair::generate(&mut OsRng);
        let spt_mega = Keypair::generate(&mut OsRng);
        let vault = Keypair::generate(&mut OsRng);
        let vault_mega = Keypair::generate(&mut OsRng);
        let vault_stake = Keypair::generate(&mut OsRng);
        let vault_stake_mega = Keypair::generate(&mut OsRng);
        let vault_pw = Keypair::generate(&mut OsRng);
        let vault_pw_mega = Keypair::generate(&mut OsRng);
        let create_pool_token_ix = rpc::create_token_account_instructions(
            self.rpc(),
            spt.pubkey(),
            &r.pool_mint,
            &vault_authority,
            self.inner.payer(),
        )?;
        let create_mega_pool_token_ix = rpc::create_token_account_instructions(
            self.rpc(),
            spt_mega.pubkey(),
            &r.pool_mint_mega,
            &vault_authority,
            self.inner.payer(),
        )?;
        let create_vault_ix = rpc::create_token_account_instructions(
            self.rpc(),
            vault.pubkey(),
            &r.mint,
            &vault_authority,
            self.inner.payer(),
        )?;
        let create_vault_mega_ix = rpc::create_token_account_instructions(
            self.rpc(),
            vault_mega.pubkey(),
            &r.mega_mint,
            &vault_authority,
            self.inner.payer(),
        )?;
        let create_vault_stake_ix = rpc::create_token_account_instructions(
            self.rpc(),
            vault_stake.pubkey(),
            &r.mint,
            &vault_authority,
            self.inner.payer(),
        )?;
        let create_vault_stake_mega_ix = rpc::create_token_account_instructions(
            self.rpc(),
            vault_stake_mega.pubkey(),
            &r.mega_mint,
            &vault_authority,
            self.inner.payer(),
        )?;
        let create_vault_pw_ix = rpc::create_token_account_instructions(
            self.rpc(),
            vault_pw.pubkey(),
            &r.mint,
            &vault_authority,
            self.inner.payer(),
        )?;
        let create_vault_pw_mega_ix = rpc::create_token_account_instructions(
            self.rpc(),
            vault_pw_mega.pubkey(),
            &r.mega_mint,
            &vault_authority,
            self.inner.payer(),
        )?;

        let mut instructions0 = vec![];
        instructions0.extend_from_slice(&create_pool_token_ix);
        instructions0.extend_from_slice(&create_mega_pool_token_ix);
        instructions0.extend_from_slice(&create_vault_ix);
        instructions0.extend_from_slice(&create_vault_mega_ix);

        let mut instructions1 = vec![];
        instructions1.extend_from_slice(&create_vault_stake_ix);
        instructions1.extend_from_slice(&create_vault_stake_mega_ix);
        instructions1.extend_from_slice(&create_vault_pw_ix);
        instructions1.extend_from_slice(&create_vault_pw_mega_ix);

        let signers0 = vec![self.inner.payer(), &spt, &spt_mega, &vault, &vault_mega];
        let signers1 = vec![
            self.inner.payer(),
            &vault_stake,
            &vault_stake_mega,
            &vault_pw,
            &vault_pw_mega,
        ];
        let (recent_hash, _fee_calc) = self.rpc().get_recent_blockhash()?;

        let tx0 = Transaction::new_signed_with_payer(
            &instructions0,
            Some(&self.inner.payer().pubkey()),
            &signers0,
            recent_hash,
        );
        let tx1 = Transaction::new_signed_with_payer(
            &instructions1,
            Some(&self.inner.payer().pubkey()),
            &signers1,
            recent_hash,
        );

        let _sig0 = self
            .inner
            .rpc()
            .send_and_confirm_transaction_with_spinner_and_config(
                &tx0,
                self.inner.options().commitment,
                self.inner.options().tx,
            )
            .map_err(ClientError::RpcError)?;
        let _sig1 = self
            .inner
            .rpc()
            .send_and_confirm_transaction_with_spinner_and_config(
                &tx1,
                self.inner.options().commitment,
                self.inner.options().tx,
            )
            .map_err(ClientError::RpcError)?;
        Ok(BalanceAccounts {
            spt,
            spt_mega,
            vault,
            vault_mega,
            vault_stake,
            vault_stake_mega,
            vault_pw,
            vault_pw_mega,
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

        // Dummy account to pass into the instruction, since it conforms to the
        // lockup program's whitelist withdraw/deposit interface.
        let dummy_account_meta = AccountMeta::new_readonly(sysvar::clock::ID, false);

        let vault = self.vault_for(&member, &depositor, false)?;
        let vault_acc = rpc::get_token_account::<TokenAccount>(self.rpc(), &vault)?;
        let accounts = vec![
            // Whitelist relay interface,
            dummy_account_meta,
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

        // Dummy account to pass into the instruction, since it conforms to the
        // lockup program's whitelist withdraw/deposit interface.
        let dummy_account_meta = AccountMeta::new_readonly(sysvar::clock::ID, false);

        let vault = self.vault_for(&member, &depositor, false)?;
        let vault_acc = rpc::get_token_account::<TokenAccount>(self.rpc(), &vault)?;
        let accounts = vec![
            // Whitelist relay interface.
            dummy_account_meta,
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
            balance_id,
        } = req;
        let r = self.registrar(&registrar)?;
        let m = self.member(&member)?;
        let b = m
            .balances
            .iter()
            .filter(|b| b.owner == balance_id)
            .collect::<Vec<&BalanceSandbox>>();
        let balances = b.first().unwrap();

        let (pool_mint, spt, vault, vault_stake) = match mega {
            false => (
                r.pool_mint,
                balances.spt,
                balances.vault,
                balances.vault_stake,
            ),
            true => (
                r.pool_mint_mega,
                balances.spt_mega,
                balances.vault_mega,
                balances.vault_stake_mega,
            ),
        };

        let vault_authority = self.vault_authority(&registrar)?;

        let accounts = vec![
            AccountMeta::new(member, false),
            AccountMeta::new_readonly(beneficiary.pubkey(), true),
            AccountMeta::new(entity, false),
            AccountMeta::new_readonly(registrar, false),
            AccountMeta::new(vault, false),
            AccountMeta::new_readonly(vault_authority, false),
            AccountMeta::new(vault_stake, false),
            AccountMeta::new(pool_mint, false),
            AccountMeta::new(spt, false),
            AccountMeta::new_readonly(r.reward_event_q, false),
            AccountMeta::new_readonly(solana_sdk::sysvar::clock::ID, false),
            AccountMeta::new_readonly(spl_token::ID, false),
            AccountMeta::new_readonly(m.balances[0].owner, false),
            AccountMeta::new_readonly(m.balances[0].spt, false),
            AccountMeta::new_readonly(m.balances[0].spt_mega, false),
            AccountMeta::new_readonly(m.balances[1].owner, false),
            AccountMeta::new_readonly(m.balances[1].spt, false),
            AccountMeta::new_readonly(m.balances[1].spt_mega, false),
        ];

        let signers = [self.payer(), beneficiary];

        let tx =
            self.inner
                .stake_with_signers(&signers, &accounts, pool_token_amount, balance_id)?;

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
            balance_id,
        } = req;
        let pending_withdrawal = Keypair::generate(&mut OsRng);

        let r = self.registrar(&registrar)?;
        let m = self.member(&member)?;
        let b = m
            .balances
            .iter()
            .filter(|b| b.owner == balance_id)
            .collect::<Vec<&BalanceSandbox>>();
        let balances = b.first().unwrap();

        let (pool_mint, spt, vault_pw, vault_stake) = match mega {
            false => (
                r.pool_mint,
                balances.spt,
                balances.vault_pending_withdrawal,
                balances.vault_stake,
            ),
            true => (
                r.pool_mint_mega,
                balances.spt_mega,
                balances.vault_pending_withdrawal_mega,
                balances.vault_stake_mega,
            ),
        };
        let vault_authority = self.vault_authority(&registrar)?;

        let accs = vec![
            AccountMeta::new(pending_withdrawal.pubkey(), false),
            //
            AccountMeta::new(member, false),
            AccountMeta::new_readonly(beneficiary.pubkey(), true),
            AccountMeta::new(entity, false),
            AccountMeta::new_readonly(registrar, false),
            AccountMeta::new(vault_pw, false),
            AccountMeta::new_readonly(vault_authority, false),
            AccountMeta::new(vault_stake, false),
            AccountMeta::new(pool_mint, false),
            AccountMeta::new(spt, false),
            AccountMeta::new_readonly(solana_sdk::sysvar::clock::ID, false),
            AccountMeta::new_readonly(spl_token::ID, false),
            AccountMeta::new_readonly(solana_sdk::sysvar::rent::ID, false),
            AccountMeta::new_readonly(r.reward_event_q, false),
            AccountMeta::new_readonly(m.balances[0].owner, false),
            AccountMeta::new_readonly(m.balances[0].spt, false),
            AccountMeta::new_readonly(m.balances[0].spt_mega, false),
            AccountMeta::new_readonly(m.balances[1].owner, false),
            AccountMeta::new_readonly(m.balances[1].spt, false),
            AccountMeta::new_readonly(m.balances[1].spt_mega, false),
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
                balance_id,
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
        let m = self.member(&member)?;
        let pw = self.pending_withdrawal(&pending_withdrawal)?;
        let b = m
            .balances
            .iter()
            .filter(|b| b.owner == pw.balance_id)
            .collect::<Vec<&BalanceSandbox>>();
        let balances = b.first().unwrap();

        let mega = pw.pool == self.registrar(&registrar)?.mega_mint;
        let (vault, vault_pw) = match mega {
            false => (balances.vault, balances.vault_pending_withdrawal),
            true => (balances.vault_mega, balances.vault_pending_withdrawal_mega),
        };

        let vault_authority = self.vault_authority(&registrar)?;

        let accs = vec![
            AccountMeta::new(pending_withdrawal, false),
            AccountMeta::new(member, false),
            AccountMeta::new(vault, false),
            AccountMeta::new(vault_pw, false),
            AccountMeta::new(vault_authority, false),
            AccountMeta::new_readonly(beneficiary.pubkey(), true),
            AccountMeta::new(entity, false),
            AccountMeta::new_readonly(spl_token::ID, false),
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
        let r = self.registrar(&registrar)?;
        let m = self.member(&member)?;
        let accs = vec![
            AccountMeta::new(member, false),
            AccountMeta::new_readonly(beneficiary.pubkey(), true),
            AccountMeta::new_readonly(registrar, false),
            AccountMeta::new(entity, false),
            AccountMeta::new(new_entity, false),
            AccountMeta::new_readonly(solana_sdk::sysvar::clock::ID, false),
            AccountMeta::new_readonly(self.vault_authority(&registrar)?, false),
            AccountMeta::new_readonly(r.reward_event_q, false),
            AccountMeta::new_readonly(m.balances[0].owner, false),
            AccountMeta::new_readonly(m.balances[0].spt, false),
            AccountMeta::new_readonly(m.balances[0].spt_mega, false),
            AccountMeta::new_readonly(m.balances[1].owner, false),
            AccountMeta::new_readonly(m.balances[1].spt, false),
            AccountMeta::new_readonly(m.balances[1].spt_mega, false),
        ];
        let tx = self
            .inner
            .switch_entity_with_signers(&[self.payer(), beneficiary], &accs)?;
        Ok(SwitchEntityResponse { tx })
    }

    pub fn expire_unlocked_reward(
        &self,
        req: ExpireUnlockedRewardRequest,
    ) -> Result<ExpireUnlockedRewardResponse, ClientError> {
        let ExpireUnlockedRewardRequest {
            token,
            vendor,
            registrar,
        } = req;
        let vendor_acc = self.unlocked_vendor(&vendor)?;
        let vendor_va = Pubkey::create_program_address(
            &[registrar.as_ref(), vendor.as_ref(), &[vendor_acc.nonce]],
            self.program(),
        )
        .map_err(|_| ClientError::Any(anyhow::anyhow!("invalid vendor vault authority")))?;
        let accs = vec![
            AccountMeta::new_readonly(self.payer().pubkey(), true),
            AccountMeta::new(token, false),
            AccountMeta::new(vendor, false),
            AccountMeta::new(vendor_acc.vault, false),
            AccountMeta::new_readonly(vendor_va, false),
            AccountMeta::new_readonly(registrar, false),
            AccountMeta::new_readonly(spl_token::ID, false),
            AccountMeta::new_readonly(sysvar::clock::ID, false),
        ];
        let tx = self.inner.expire_unlocked_reward(&accs)?;
        Ok(ExpireUnlockedRewardResponse { tx })
    }

    pub fn expire_locked_reward(
        &self,
        req: ExpireLockedRewardRequest,
    ) -> Result<ExpireLockedRewardResponse, ClientError> {
        let ExpireLockedRewardRequest {
            token,
            vendor,
            registrar,
        } = req;
        let vendor_acc = self.locked_vendor(&vendor)?;
        let vendor_va = Pubkey::create_program_address(
            &[registrar.as_ref(), vendor.as_ref(), &[vendor_acc.nonce]],
            self.program(),
        )
        .map_err(|_| ClientError::Any(anyhow::anyhow!("invalid vendor vault authority")))?;
        let accs = vec![
            AccountMeta::new_readonly(self.payer().pubkey(), true),
            AccountMeta::new(token, false),
            AccountMeta::new(vendor, false),
            AccountMeta::new(vendor_acc.vault, false),
            AccountMeta::new_readonly(vendor_va, false),
            AccountMeta::new_readonly(registrar, false),
            AccountMeta::new_readonly(spl_token::ID, false),
            AccountMeta::new_readonly(sysvar::clock::ID, false),
        ];
        let tx = self.inner.expire_locked_reward(&accs)?;
        Ok(ExpireLockedRewardResponse { tx })
    }
}

// Account accessors.
impl Client {
    pub fn registrar(&self, registrar: &Pubkey) -> Result<Registrar, ClientError> {
        rpc::get_account::<Registrar>(self.rpc(), registrar).map_err(Into::into)
    }

    pub fn entity(&self, entity: &Pubkey) -> Result<Entity, ClientError> {
        rpc::get_account_unchecked::<Entity>(self.rpc(), entity).map_err(Into::into)
    }
    pub fn vault_authority(&self, registrar: &Pubkey) -> Result<Pubkey, ClientError> {
        let r = self.registrar(registrar)?;
        Pubkey::create_program_address(&vault::signer_seeds(registrar, &r.nonce), self.program())
            .map_err(|_| ClientError::Any(anyhow::anyhow!("invalid vault authority")))
    }
    pub fn member(&self, member: &Pubkey) -> Result<Member, ClientError> {
        rpc::get_account::<Member>(self.rpc(), &member).map_err(Into::into)
    }
    pub fn member_seed() -> &'static str {
        "srm:registry:member"
    }

    pub fn vault_for(
        &self,
        member: &Pubkey,
        depositor: &Pubkey,
        locked: bool, // todo use balance_id instead
    ) -> Result<Pubkey, ClientError> {
        let depositor = rpc::get_token_account::<TokenAccount>(self.rpc(), depositor)?;
        let member = self.member(member)?;
        let balances = match locked {
            true => &member.balances[1],
            false => &member.balances[0],
        };

        let vault = rpc::get_token_account::<TokenAccount>(self.rpc(), &balances.vault)?;
        if vault.mint == depositor.mint {
            return Ok(balances.vault);
        }

        let mega_vault = rpc::get_token_account::<TokenAccount>(self.rpc(), &balances.vault_mega)?;
        if mega_vault.mint == depositor.mint {
            return Ok(balances.vault_mega);
        }
        Err(ClientError::Any(anyhow::anyhow!("invalid depositor mint")))
    }

    pub fn current_deposit_vault(
        &self,
        member: &Pubkey,
        locked: bool,
    ) -> Result<TokenAccount, ClientError> {
        let m = self.member(member)?;
        let balances = match locked {
            true => &m.balances[1],
            false => &m.balances[0],
        };
        rpc::get_token_account::<TokenAccount>(self.rpc(), &balances.vault).map_err(Into::into)
    }

    pub fn current_deposit_mega_vault(
        &self,
        member: &Pubkey,
        locked: bool,
    ) -> Result<TokenAccount, ClientError> {
        let m = self.member(member)?;
        let balances = match locked {
            true => &m.balances[1],
            false => &m.balances[0],
        };
        rpc::get_token_account::<TokenAccount>(self.rpc(), &balances.vault_mega).map_err(Into::into)
    }

    pub fn pool_token(
        &self,
        member: &Pubkey,
        locked: bool,
    ) -> Result<ProgramAccount<TokenAccount>, ClientError> {
        let m = self.member(member)?;
        let balances = match locked {
            true => &m.balances[1],
            false => &m.balances[0],
        };
        let account = rpc::get_token_account(self.rpc(), &balances.spt)?;
        Ok(ProgramAccount {
            public_key: balances.spt_mega,
            account,
        })
    }

    pub fn mega_pool_token(
        &self,
        member: &Pubkey,
        locked: bool,
    ) -> Result<ProgramAccount<TokenAccount>, ClientError> {
        let m = self.member(member)?;
        let balances = match locked {
            true => &m.balances[1],
            false => &m.balances[0],
        };
        let account = rpc::get_token_account(self.rpc(), &balances.spt_mega)?;
        Ok(ProgramAccount {
            public_key: balances.spt_mega,
            account,
        })
    }

    pub fn stake_pool_asset_vault(
        &self,
        member: &Pubkey,
        locked: bool,
    ) -> Result<TokenAccount, ClientError> {
        let m = self.member(member)?;
        let balances = match locked {
            true => &m.balances[1],
            false => &m.balances[0],
        };
        rpc::get_token_account::<TokenAccount>(self.rpc(), &balances.vault_stake)
            .map_err(Into::into)
    }

    pub fn stake_mega_pool_asset_vault(
        &self,
        member: &Pubkey,
        locked: bool,
    ) -> Result<TokenAccount, ClientError> {
        let m = self.member(member)?;
        let balances = match locked {
            true => &m.balances[1],
            false => &m.balances[0],
        };
        rpc::get_token_account::<TokenAccount>(self.rpc(), &balances.vault_stake_mega)
            .map_err(Into::into)
    }

    pub fn pending_withdrawal_vault(
        &self,
        member: &Pubkey,
        locked: bool,
    ) -> Result<TokenAccount, ClientError> {
        let m = self.member(member)?;
        let balances = match locked {
            true => &m.balances[1],
            false => &m.balances[0],
        };
        rpc::get_token_account::<TokenAccount>(self.rpc(), &balances.vault_pending_withdrawal)
            .map_err(Into::into)
    }

    pub fn pending_withdrawal(&self, pw: &Pubkey) -> Result<PendingWithdrawal, ClientError> {
        rpc::get_account::<PendingWithdrawal>(self.rpc(), pw).map_err(Into::into)
    }

    pub fn unlocked_vendor(&self, v: &Pubkey) -> Result<UnlockedRewardVendor, ClientError> {
        rpc::get_account::<UnlockedRewardVendor>(self.rpc(), v).map_err(Into::into)
    }

    pub fn locked_vendor(&self, v: &Pubkey) -> Result<LockedRewardVendor, ClientError> {
        rpc::get_account::<LockedRewardVendor>(self.rpc(), v).map_err(Into::into)
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

fn create_metadata_instructions(
    client: &RpcClient,
    payer: &Pubkey,
    metadata: &Keypair,
    program_id: &Pubkey,
    entity: &Pubkey,
    name: String,
    about: String,
    image_url: String,
) -> Vec<Instruction> {
    let metadata_size = {
        // 280 chars max.
        let max_name = "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
						.to_string();
        let max_about = max_name.clone();
        let max_image_url = max_name.clone();
        let md = serum_meta_entity::accounts::Metadata {
            initialized: false,
            entity: Pubkey::new_from_array([0; 32]),
            authority: *payer,
            name: max_name,
            about: max_about,
            image_url: max_image_url,
            chat: Pubkey::new_from_array([0; 32]),
        };
        md.size().unwrap()
    };
    let lamports = client
        .get_minimum_balance_for_rent_exemption(metadata_size as usize)
        .unwrap();
    let create_metadata_instr = solana_sdk::system_instruction::create_account(
        payer,
        &metadata.pubkey(),
        lamports,
        metadata_size as u64,
        program_id,
    );

    let initialize_metadata_instr = {
        let accounts = vec![AccountMeta::new(metadata.pubkey(), false)];
        let instr = serum_meta_entity::instruction::MetaEntityInstruction::Initialize {
            entity: *entity,
            authority: *payer,
            name,
            about,
            image_url,
            chat: Pubkey::new_from_array([0; 32]),
        };
        let mut data = vec![0u8; instr.size().unwrap() as usize];
        serum_meta_entity::instruction::MetaEntityInstruction::pack(instr, &mut data).unwrap();
        Instruction {
            program_id: *program_id,
            accounts,
            data,
        }
    };

    vec![create_metadata_instr, initialize_metadata_instr]
}

pub struct InitializeRequest {
    pub registrar_authority: Pubkey,
    pub withdrawal_timelock: i64,
    pub deactivation_timelock: i64,
    pub max_stake_per_entity: u64,
    pub mint: Pubkey,
    pub mega_mint: Pubkey,
    pub stake_rate: u64,
    pub stake_rate_mega: u64,
}

pub struct InitializeResponse {
    pub tx: Signature,
    pub registrar: Pubkey,
    pub reward_event_q: Pubkey,
    pub nonce: u8,
}

pub struct CreateEntityRequest<'a> {
    pub node_leader: &'a Keypair,
    pub registrar: Pubkey,
    pub metadata: Option<EntityMetadata>,
}

pub struct EntityMetadata {
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

pub struct UpdateEntityMetadataRequest {
    pub name: Option<String>,
    pub about: Option<String>,
    pub image_url: Option<String>,
    pub meta_entity_pid: Pubkey,
    pub entity: Pubkey,
}

pub struct UpdateEntityMetadataResponse {
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
    pub balance_id: Pubkey,
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
    pub balance_id: Pubkey,
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

pub struct ExpireUnlockedRewardRequest {
    pub token: Pubkey,
    pub vendor: Pubkey,
    pub registrar: Pubkey,
}

pub struct ExpireUnlockedRewardResponse {
    pub tx: Signature,
}

pub struct ExpireLockedRewardRequest {
    pub token: Pubkey,
    pub vendor: Pubkey,
    pub registrar: Pubkey,
}

pub struct ExpireLockedRewardResponse {
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

struct BalanceAccounts {
    spt: Keypair,
    spt_mega: Keypair,
    vault: Keypair,
    vault_mega: Keypair,
    vault_stake: Keypair,
    vault_stake_mega: Keypair,
    vault_pw: Keypair,
    vault_pw_mega: Keypair,
}
