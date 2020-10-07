//! The client_ext module extends the auto-generated program client.

use crate::accounts::vesting;
use crate::accounts::{Safe, Whitelist};
use serum_common::client::rpc;
use serum_common::pack::Pack;
use solana_client_gen::prelude::*;
use solana_client_gen::solana_sdk;
use solana_client_gen::solana_sdk::instruction::AccountMeta;
use solana_client_gen::solana_sdk::pubkey::Pubkey;
use solana_client_gen::solana_sdk::signers::Signers;
use solana_client_gen::solana_sdk::system_instruction;

// TODO: Use deterministic derived addresses for all accounts associated with
//       the program. This will allow users to query on chain data with nothing
//       but the program address and "instance" address (e.g. the "Mint" in SPL
//       tokens).
//
//       * https://docs.rs/solana-sdk/1.3.12/solana_sdk/pubkey/struct.Pubkey.html#method.create_with_seed
//       * https://docs.rs/solana-sdk/1.3.12/solana_sdk/system_instruction/fn.create_account_with_seed.html
//
//
//       Would be nice to have macro support for this in the future.
//
//       Right now all tests just use randomly generated accounts, which is
//       find for testing the program, but doesn't provide a robust client
//       experience.
solana_client_gen_extension! {
    impl Client {
        /// Does complete initialization of the safe.
        ///
        /// Assumes:
        ///
        ///   * The coin to be deposited (SRM) is already minted.
        ///   * The program is already deployed on chain.
        ///
        pub fn create_all_accounts_and_initialize(
            &self,
            accounts: &[AccountMeta],
            srm_mint: &Pubkey,
            safe_authority: &Pubkey,
        ) -> Result<InitializeResponse, ClientError> {
            // Build the data dependent addresses.
            //
            // The safe instance requires a nonce for it's token vault, which
            // uses a program-derived address to "sign" transactions and
            // manage funds within the program.
            let safe_acc = Keypair::generate(&mut OsRng);
            let (safe_vault_authority, nonce) = Pubkey::find_program_address(
                &[safe_acc.pubkey().as_ref()],
                self.program(),
            );

            // Create and initialize the vault, owned by a program-derived-address.
            let safe_srm_vault = serum_common::client::rpc::create_token_account(
                self.rpc(),
                &srm_mint,
                &safe_vault_authority,
                self.payer(),
            ).map_err(|e| ClientError::RawError(e.to_string()))?;

            // Now build the final transaction.
            let wl_kp = Keypair::generate(&mut OsRng);
            let instructions = {
                let create_safe_acc_instr = {
                    let lamports = self
                        .rpc()
                        .get_minimum_balance_for_rent_exemption(
                            Safe::default().size().unwrap() as usize
                        )
                        .map_err(ClientError::RpcError)?;
                    system_instruction::create_account(
                        &self.payer().pubkey(),
                        &safe_acc.pubkey(),
                        lamports,
                        Safe::default().size().unwrap(),
                        self.program(),
                    )
                };
                let create_whitelist_acc_instr = {
                    let lamports = self
                        .rpc()
                        .get_minimum_balance_for_rent_exemption(
                            Whitelist::default().size().unwrap() as usize
                        )
                        .map_err(ClientError::RpcError)?;
                    system_instruction::create_account(
                        &self.payer().pubkey(),
                        &wl_kp.pubkey(),
                        lamports,
                        Whitelist::default().size().unwrap(),
                        self.program(),
                    )
                };

                let mut accounts = accounts.to_vec();
                accounts.insert(0, AccountMeta::new(safe_acc.pubkey(), false));
                accounts.insert(1, AccountMeta::new(wl_kp.pubkey(), false));
                accounts.insert(2, AccountMeta::new_readonly(*srm_mint, false));

                let initialize_instr = super::instruction::initialize(
                    *self.program(),
                    &accounts,
                    *safe_authority,
                    nonce,
                );
                vec![create_safe_acc_instr, create_whitelist_acc_instr, initialize_instr]
            };

            let tx = {
                let (recent_hash, _fee_calc) = self
                    .rpc()
                    .get_recent_blockhash()
                    .map_err(|e| ClientError::RawError(e.to_string()))?;
                let signers = vec![self.payer(), &safe_acc, &wl_kp];
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
                    safe_acc,
                    vault_acc_authority: safe_vault_authority,
                    vault_acc: safe_srm_vault,
                    whitelist: wl_kp.pubkey(),
                    nonce,
                })
        }
        pub fn create_vesting_account(
            &self,
            depositor: &Pubkey,
            safe_acc: &Pubkey,
            safe_vault: &Pubkey,
            safe_vault_authority: &Pubkey,
            vesting_acc_beneficiary: &Pubkey,
            end_slot: u64,
            period_count: u64,
            deposit_amount: u64,
            mint_decimals: u8,
        ) -> Result<(Signature, Keypair, Pubkey), ClientError> {
            let mint_kp = Keypair::generate(&mut OsRng);
            let mint_authority = safe_vault_authority;
            let _tx_sig = rpc::create_and_init_mint(
                self.rpc(),
                self.payer(),
                &mint_kp,
                &safe_vault_authority,
                mint_decimals,
            ).map_err(|e| ClientError::RawError(e.to_string()))?;

            let deposit_accs = [
                AccountMeta::new(*depositor, false),
                AccountMeta::new(self.payer().pubkey(), true), // Owner of depositor.
                AccountMeta::new(*safe_vault, false),
                AccountMeta::new(*safe_acc, false),
                AccountMeta::new(mint_kp.pubkey(), false),
                AccountMeta::new_readonly(*safe_vault_authority, false),
                AccountMeta::new_readonly(spl_token::ID, false),
                AccountMeta::new_readonly(solana_sdk::sysvar::rent::ID, false),
                AccountMeta::new_readonly(solana_sdk::sysvar::clock::ID, false),
            ];
            self
                .create_account_and_deposit(
                    &deposit_accs,
                    *vesting_acc_beneficiary,
                    end_slot,
                    period_count,
                    deposit_amount,
                )
                .map(|(sig, kp)| (sig, kp, mint_kp.pubkey()))
        }
    }

    pub struct InitializeResponse {
        pub signature: Signature,
        pub safe_acc: Keypair,
        pub vault_acc: Keypair,
        pub vault_acc_authority: Pubkey,
        pub whitelist: Pubkey,
        pub nonce: u8,
    }
}
