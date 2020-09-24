//! The client_ext module extends the auto-generated program client.

use crate::accounts::{MintReceipt, Safe};
use serum_common::pack::Pack;
use solana_client_gen::prelude::*;
use solana_client_gen::solana_sdk;
use solana_client_gen::solana_sdk::instruction::AccountMeta;
use solana_client_gen::solana_sdk::pubkey::Pubkey;
use spl_token::pack::Pack as TokenPack;

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
    use solana_client_gen::solana_sdk::signers::Signers;

    pub struct SafeInitialization {
        pub signature: Signature,
        pub safe_acc: Keypair,
        pub vault_acc: Keypair,
        pub vault_acc_authority: Pubkey,
        pub nonce: u8,
    }

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
        ) -> Result<SafeInitialization, ClientError> {
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

                let mut accounts = accounts.to_vec();
                accounts.insert(0, AccountMeta::new(safe_acc.pubkey(), false));
                accounts.insert(1, AccountMeta::new_readonly(*srm_mint, false));

                let initialize_instr = super::instruction::initialize(
                    *self.program(),
                    &accounts,
                    *safe_authority,
                    nonce,
                );
                vec![create_safe_acc_instr, initialize_instr]
            };

            let tx = {
                let (recent_hash, _fee_calc) = self
                    .rpc()
                    .get_recent_blockhash()
                    .map_err(|e| ClientError::RawError(e.to_string()))?;
                let signers = vec![self.payer(), &safe_acc];
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
                .map(|sig| SafeInitialization {
                    signature: sig,
                    safe_acc,
                    vault_acc_authority: safe_vault_authority,
                    vault_acc: safe_srm_vault,
                    nonce,
                })
        }

        /// Creates a multi instruction transaction to mint NFT tokens backed
        /// by Safe vesting accounts. For each `mint_count` number NFTs to mint,
        /// the transaction executes:
        ///
        /// * system::create_account for the token mint representing the NFT
        /// * system::create_account for the token account holding the NFT.
        /// * safe::mint which takes the accounts from the previous instructions
        ///   to issue the mint.
        ///
        /// The motivation here is that, given an lSRM NFT mint, the program needs to
        /// know if it should acknowledge the mint by printing a new receipt
        /// (by setting the initialization flag).
        ///
        /// It can do this by either having a mapping in storage and checking
        /// it when the mint instruction is run, or, it can *only* print receipts
        /// for NFT mints that are not yet initialized (and initialize the mint
        /// itself).
        ///
        /// However, inorder to safely initialize a mint, we must have the mint's
        /// create account instruction run in the same transaction as the initialize
        /// mint instruction.
        ///
        /// The lSRM receipt instructions are sent as transactions immediately
        /// before the mint transaction, because otherwise the Solana
        /// transactions will be too large.
        pub fn create_nfts_and_mint_locked_with_signers(
            &self,
            mint_count: usize,
            nft_token_acc_owner: &Pubkey,
            signers: Vec<&Keypair>,
            accounts: Vec<AccountMeta>,
        ) -> Result<(Signature, Vec<Lsrm>), ClientError> {
            // Create the receipt accounts (in separate transactions because
            // we will go over the transaction size limit).
            let mut receipt_keys = self.send_create_receipts(mint_count)?;

            // Build the transaction.
            let (
                tx,
                mut nft_mint_keys,
                mut nft_token_acc_keys,
            ) = {
                // Rescope lifetime to this block.
                let mut signers = signers;

                let mut instructions = vec![];
                let mut nft_mint_keys = vec![];
                let mut nft_token_acc_keys = vec![];

                // Build and collect a batch of instructions and account keys
                // for each NFT we want to mint.
                {
                    let batches = receipt_keys
                        .iter()
                        .map(|receipt|  {
                            self.mint_instructions(
                                nft_token_acc_owner,
                                accounts.clone(),
                                receipt,
                            )
                        });
                    for b in batches {
                        let (instr_batch, mint_key, token_key) = b?;
                        instructions.extend_from_slice(&instr_batch);
                        nft_mint_keys.push(mint_key);
                        nft_token_acc_keys.push(token_key);
                    }
                }

                // Collect signers on the entire tx.
                for k in 0..nft_mint_keys.len() {
                    signers.push(&nft_mint_keys[k]);
                    signers.push(&nft_token_acc_keys[k]);
                }

                // Create the tx.
                let (recent_hash, _fee_calc) = self.rpc().get_recent_blockhash()?;
                let tx = Transaction::new_signed_with_payer(
                    &instructions,
                    Some(&self.payer().pubkey()),
                    &signers,
                    recent_hash,
                );
                (tx, nft_mint_keys, nft_token_acc_keys)
            };

            // Execute it.
            self
                .rpc
                .send_and_confirm_transaction_with_spinner_and_config(
                    &tx,
                    self.opts.commitment,
                    self.opts.tx,
                )
                .map_err(ClientError::RpcError)
                .map(|sig| {
                    // Format a nice return value.
                    let mut lsrm_nfts = vec![];
                    for _ in 0..nft_mint_keys.len() {
                        let mint = nft_mint_keys.pop().unwrap();
                        let token_acc = nft_token_acc_keys.pop().unwrap();
                        let receipt = receipt_keys.pop().unwrap();
                        lsrm_nfts.push(Lsrm {
                            mint,
                            token_acc,
                            receipt: receipt.pubkey(),
                        });
                    }
                    (sig, lsrm_nfts)
                })
        }

        // TODO: Batch the account create instructions to minimize RPCs.
        //
        //       Ideally we'd just put these into the same transaction as the mint
        //       but the transaction gets too large.
        fn send_create_receipts(&self, mint_count: usize) -> Result<Vec<Keypair>, ClientError> {
            let mut receipts = vec![];

            for _ in 0..mint_count {
                let kp = serum_common::client::rpc::create_account_rent_exempt(
                    self.rpc(),
                    &self.payer(),
                    MintReceipt::default().size().unwrap() as usize,
                    &self.program(),
                ).map_err(|e| ClientError::RawError(e.to_string()))?;

                receipts.push(kp);
            }

            Ok(receipts)
        }

        // Returns the 3-batch of instructions and the created account keys
        // used to create a single Safe backed NFT mint.
        //
        //   1) spl::create-mint
        //   2) spl::create-token-account
        //   3) safe::mint.
        //
        fn mint_instructions(
            &self,
            nft_token_acc_owner: &Pubkey,
            accounts: Vec<AccountMeta>,
            receipt: &Keypair,
        ) -> Result<(Vec<Instruction>, Keypair, Keypair), ClientError> {
            let lamports_mint = self.rpc().get_minimum_balance_for_rent_exemption(
                spl_token::state::Mint::LEN,
            )?;
            let lamports_token_acc = self.rpc().get_minimum_balance_for_rent_exemption(
                spl_token::state::Account::LEN,
            )?;

            // The NFT Mint to intialize.
            let mint = Keypair::generate(&mut OsRng);
            let create_mint_acc_instr = solana_sdk::system_instruction::create_account(
                &self.payer().pubkey(),
                &mint.pubkey(),
                lamports_mint,
                spl_token::state::Mint::LEN as u64,
                &spl_token::ID,
            );

            // The token Account to hold the NFT.
            let token_acc = Keypair::generate(&mut OsRng);
            let create_token_acc_instr = solana_sdk::system_instruction::create_account(
                &self.payer().pubkey(),
                &token_acc.pubkey(),
                lamports_token_acc,
                spl_token::state::Account::LEN as u64,
                &spl_token::ID,
            );

            // Push the accounts for the eventual mint_locked instruction.
            let mut accounts = accounts;
            accounts.push(AccountMeta::new(mint.pubkey(), true));
            accounts.push(AccountMeta::new(token_acc.pubkey(), true));
            accounts.push(AccountMeta::new(receipt.pubkey(), false));

            // Create the instruction.
            let mint_instr = super::instruction::mint_locked(
                *self.program(),
                &accounts,
                *nft_token_acc_owner,
            );

            let instructions = vec![
                create_mint_acc_instr,
                create_token_acc_instr,
                mint_instr,
            ];

            Ok((instructions, mint, token_acc))
        }
    }

    /// Lsrm defines the required keys to redeem and otherwise use lSRM.
    pub struct Lsrm {
        /// The SPL token mint representing the NFT instance.
        pub mint: Keypair,
        /// The only account allowed to own the mint (other than valid
        /// locked programs).
        pub token_acc: Keypair,
        /// The receipt account address. Required upon redemption to prove
        /// to the program this is a valid lSRM mint.
        pub receipt: Pubkey,
    }
}
