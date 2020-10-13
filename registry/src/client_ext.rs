use crate::accounts::member;
use crate::accounts::registrar;
use serum_common::pack::Pack;
use solana_client_gen::prelude::*;
use solana_client_gen::solana_sdk;
use solana_client_gen::solana_sdk::instruction::AccountMeta;
use solana_client_gen::solana_sdk::pubkey::Pubkey;
use solana_client_gen::solana_sdk::signers::Signers;
use solana_client_gen::solana_sdk::system_instruction;

solana_client_gen_extension! {
    impl Client {
        pub fn create_entity_derived(
            &self,
            leader_kp: &Keypair,
            capabilities: u32,
            stake_kind: crate::accounts::StakeKind,
        ) -> Result<(Signature, Pubkey), ClientError> {
            let entity_account_size = *crate::accounts::entity::SIZE;
            let lamports = self.rpc().get_minimum_balance_for_rent_exemption(
                entity_account_size as usize,
            )?;

            let entity_address = self.entity_address_derived(&leader_kp.pubkey())?;
            let create_acc_instr =
                solana_sdk::system_instruction::create_account_with_seed(
                    &self.payer().pubkey(),   // From (signer).
                    &entity_address,          // To.
                    &leader_kp.pubkey(),      // Base (signer).
                    Self::entity_seed(),      // Seed.
                    lamports,                 // Account start balance.
                    entity_account_size,      // Acc size.
                    &self.program(),          // Owner.
                );

            let accounts = [
                AccountMeta::new(entity_address, false),
                AccountMeta::new_readonly(leader_kp.pubkey(), true),
                AccountMeta::new_readonly(solana_sdk::sysvar::rent::ID, false),
            ];
            let create_entity_instr = super::instruction::create_entity(
                *self.program(),
                &accounts,
                capabilities,
                stake_kind,
            );
            let instructions = [
                create_acc_instr, create_entity_instr,
            ];
            let signers = [leader_kp, self.payer()];
            let (recent_hash, _fee_calc) = self
                .rpc()
                .get_recent_blockhash()?;

            let tx = Transaction::new_signed_with_payer(
                &instructions,
                Some(&self.payer().pubkey()),
                &signers,
                recent_hash,
            );

            self
                .rpc()
                .send_and_confirm_transaction_with_spinner_and_config(
                    &tx,
                    self.options().commitment,
                    self.options().tx,
                )
                .map_err(ClientError::RpcError)
                .map(|sig| (sig, entity_address))
        }

        pub fn join_entity_derived(
            &self,
            entity: Pubkey,
            beneficiary: Pubkey,
            delegate: Pubkey,
        ) -> Result<(Signature, Pubkey), ClientError> {

            let member_address = self.member_address_derived()?;

            let lamports = self.rpc().get_minimum_balance_for_rent_exemption(
                *crate::accounts::member::SIZE as usize,
            )?;

            let create_acc_instr =
                solana_sdk::system_instruction::create_account_with_seed(
                    &self.payer().pubkey(),
                    &member_address,
                    &self.payer().pubkey(),
                    Self::member_seed(),
                    lamports,
                    *crate::accounts::member::SIZE,
                    &self.program(),
                );

            let accounts = [
                AccountMeta::new(member_address, false),
                AccountMeta::new(entity, false),
                AccountMeta::new_readonly(solana_sdk::sysvar::rent::ID, false),
            ];

            let member_instr = super::instruction::join_entity(
                *self.program(),
                &accounts,
                beneficiary,
                delegate,
            );

            let instructions = [
                create_acc_instr, member_instr,
            ];
            let signers = [self.payer()];
            let (recent_hash, _fee_calc) = self
                .rpc()
                .get_recent_blockhash()?;

            let tx = Transaction::new_signed_with_payer(
                &instructions,
                Some(&self.payer().pubkey()),
                &signers,
                recent_hash,
            );

            self
                .rpc()
                .send_and_confirm_transaction_with_spinner_and_config(
                    &tx,
                    self.options().commitment,
                    self.options().tx,
                )
                .map_err(ClientError::RpcError)
                .map(|sig| (sig, member_address))
        }

        pub fn entity_address_derived(&self, leader: &Pubkey) -> Result<Pubkey, ClientError> {
            Pubkey::create_with_seed(
                leader,
                Self::entity_seed(),
                &self.program(),
            ).map_err(|e| ClientError::RawError(e.to_string()))
        }

        pub fn entity_seed() -> &'static str {
            "srm:registry:entity"
        }

        pub fn member_address_derived(&self) -> Result<Pubkey, ClientError> {
            Pubkey::create_with_seed(
                &self.payer().pubkey(),
                Self::member_seed(),
                &self.program(),
            ).map_err(|e| ClientError::RawError(e.to_string()))
        }

        pub fn member_seed() -> &'static str {
            "srm:registry:member"
        }
    }
}
