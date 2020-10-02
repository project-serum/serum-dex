use crate::accounts::entity;
use crate::accounts::stake;
use serum_common::pack::Pack;
use solana_client_gen::prelude::*;
use solana_client_gen::solana_sdk;
use solana_client_gen::solana_sdk::instruction::AccountMeta;
use solana_client_gen::solana_sdk::pubkey::Pubkey;
use solana_client_gen::solana_sdk::signers::Signers;
use solana_client_gen::solana_sdk::system_instruction;

solana_client_gen_extension! {
    impl Client {
        /// Does complete initialization of the Registry.
        ///
        /// Assumes:
        ///
        ///   * The coin to be deposited (SRM) is already minted.
        ///   * The program is already deployed on chain.
        ///
        // TODO: share this with the safe.
        pub fn create_all_accounts_and_initialize(
            &self,
            srm_mint: &Pubkey,
            msrm_mint: &Pubkey,
            registry_authority: &Pubkey,
        ) -> Result<InitializeResponse, ClientError> {
            // Build the data dependent addresses.
            //
            // The registry instance requires a nonce for it's token vault, which
            // uses a program-derived address to "sign" transactions and
            // manage funds within the program.
            let registry_acc = Keypair::generate(&mut OsRng);
            let (registry_vault_authority, nonce) = Pubkey::find_program_address(
                &[registry_acc.pubkey().as_ref()],
                self.program(),
            );

            // Create and initialize the vaults, owned by a program-derived-address.
            let registry_srm_vault = serum_common::client::rpc::create_token_account(
                self.rpc(),
                &srm_mint,
                &registry_vault_authority,
                self.payer(),
            ).map_err(|e| ClientError::RawError(e.to_string()))?;

            let registry_msrm_vault = serum_common::client::rpc::create_token_account(
                self.rpc(),
                &msrm_mint,
                &registry_vault_authority,
                self.payer(),
            ).map_err(|e| ClientError::RawError(e.to_string()))?;

            // Now build the final transaction.
            let instructions = {
                let create_registry_acc_instr = {
                    let lamports = self
                        .rpc()
                        .get_minimum_balance_for_rent_exemption(
                            crate::accounts::registry::SIZE
                        )
                        .map_err(ClientError::RpcError)?;
                    system_instruction::create_account(
                        &self.payer().pubkey(),
                        &registry_acc.pubkey(),
                        lamports,
                        crate::accounts::registry::SIZE as u64,
                        self.program(),
                    )
                };
                let accounts = [
                    AccountMeta::new(registry_acc.pubkey(), false),
                    AccountMeta::new_readonly(*srm_mint, false),
                    AccountMeta::new_readonly(*msrm_mint, false),
                    AccountMeta::new_readonly(solana_sdk::sysvar::rent::id(), false),
                ];
                let initialize_instr = super::instruction::initialize(
                    *self.program(),
                    &accounts,
                    *registry_authority,
                    nonce,
                );
                vec![create_registry_acc_instr, initialize_instr]
            };

            let tx = {
                let (recent_hash, _fee_calc) = self
                    .rpc()
                    .get_recent_blockhash()
                    .map_err(|e| ClientError::RawError(e.to_string()))?;
                let signers = vec![self.payer(), &registry_acc];
                Transaction::new_signed_with_payer(
                    &instructions,
                    Some(&self.payer().pubkey()),
                    &signers,
                    recent_hash,
                )
            };

            // Execute the transaction.
            self
                .rpc
                .send_and_confirm_transaction_with_spinner_and_config(
                    &tx,
                    self.opts.commitment,
                    self.opts.tx,
                )
                .map_err(ClientError::RpcError)
                .map(|sig| InitializeResponse {
                    signature: sig,
                    registry_acc,
                    vault_acc_authority: registry_vault_authority,
                    vault_acc: registry_srm_vault,
                    mega_vault_acc: registry_msrm_vault,
                    nonce,
                })
        }
    }
    pub struct InitializeResponse {
        pub signature: Signature,
        pub registry_acc: Keypair,
        pub vault_acc: Keypair,
        pub mega_vault_acc: Keypair,
        pub vault_acc_authority: Pubkey,
        pub nonce: u8,
    }

}
