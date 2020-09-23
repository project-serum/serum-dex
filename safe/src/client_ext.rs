//! The client_ext module extends the auto-generated program client.

use crate::accounts::{LsrmReceipt, SafeAccount};
use serum_common::pack::Pack;
use solana_client_gen::prelude::*;
use solana_client_gen::solana_sdk;
use solana_client_gen::solana_sdk::instruction::AccountMeta;
use solana_client_gen::solana_sdk::pubkey::Pubkey;
use spl_token::pack::Pack as TokenPack;

// TODO: Use deterministic derived addresses for all accounts associated with the program.
//       This will allow users to query on chain data with nothing but the program address
//       and "instance" address (e.g. the "Mint" in SPL tokens).
//
//       * https://docs.rs/solana-sdk/1.3.12/solana_sdk/pubkey/struct.Pubkey.html#method.create_with_seed
//       * https://docs.rs/solana-sdk/1.3.12/solana_sdk/system_instruction/fn.create_account_with_seed.html
//
//
//       Would be nice to have macro support for this in the future.
//
//       Right now all tests just use randomly generated accounts.
solana_client_gen_extension! {
    use solana_client_gen::solana_sdk::signers::Signers;

    pub struct SafeInitialization {
        pub signature: Signature,
        pub safe_account: Keypair,
        pub vault_account: Keypair,
        pub vault_account_authority: Pubkey,
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
            let safe_account = Keypair::generate(&mut OsRng);
            let (safe_vault_authority, nonce) = Pubkey::find_program_address(
                &[safe_account.pubkey().as_ref()],
                self.program(),
            );

            // Create and initialize the vault, owned by a program-derived-address.
            let safe_srm_vault = serum_common::client::rpc::create_spl_account(
                self.rpc(),
                &srm_mint,
                &safe_vault_authority,
                self.payer(),
            ).map_err(|e| ClientError::RawError(e.to_string()))?;

            // Now build the final transaction.
            let instructions = {
                let create_safe_account_instr = {
                    let lamports = self
                        .rpc()
                        .get_minimum_balance_for_rent_exemption(SafeAccount::size().unwrap() as usize)
                        .map_err(|e| ClientError::RpcError(e))?;
                    system_instruction::create_account(
                        &self.payer().pubkey(),
                        &safe_account.pubkey(),
                        lamports,
                        SafeAccount::size().unwrap(),
                        self.program(),
                    )
                };

                let mut accounts = accounts.to_vec();
                accounts.insert(0, AccountMeta::new(safe_account.pubkey(), false));

                let initialize_instr = super::instruction::initialize(
                    *self.program(),
                    &accounts,
                    *srm_mint,
                    *safe_authority,
                    nonce,
                );
                vec![create_safe_account_instr, initialize_instr]
            };

            let tx = {
                let (recent_hash, _fee_calc) = self
                    .rpc()
                    .get_recent_blockhash()
                    .map_err(|e| ClientError::RawError(e.to_string()))?;
                let signers = vec![self.payer(), &safe_account];
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
                .map_err(|e| ClientError::RpcError(e))
                .map(|sig| SafeInitialization {
                    signature: sig,
                    safe_account,
                    vault_account_authority: safe_vault_authority,
                    vault_account: safe_srm_vault,
                    nonce,
                })
        }

        /// Creates a multi instruction transaction. For each `lsrm_count` number
        /// of lSRM NFTs to mint, executes
        ///
        /// * system::create_account for the lSRM NFT SPL mint
        /// * system::create_account for the safe's lSRM receipt
        ///
        /// And *after* executing those for all NFTs, finally executes the
        /// MintLockedSrm instruction on the Safe program.
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
        /// The lSRM receipt instructions are added in for convenience.
        pub fn create_nfts_and_mint_locked_srm_with_signers(
            &self,
            // On localnet this maxes out at 2 before the transaction is too large.
            lsrm_count: usize,
            lsrm_nft_token_account_owner: &Pubkey,
            signers: Vec<&Keypair>,
            mut accounts: Vec<AccountMeta>,
        ) -> Result<(Signature, Vec<Lsrm>), ClientError> {
            // Create the lsrm-receipt accounts (in separate transactions).
            let mut lsrm_receipt_keys = self.create_lsrm_receipts(lsrm_count)?;

            // Build the mint-lsrm transaction.
            let (
                tx,
                mut lsrm_nft_mint_keys,
                mut lsrm_nft_token_account_keys,
            ) = {
                // Rescope lifetime to this block.
                let mut signers = signers;

                // Build the create_account instructions.
                let (
                    mut instructions,
                    lsrm_nft_mint_keys,
                    lsrm_nft_token_account_keys,
                ) = self.create_nfts_instructions_and_keys(
                    lsrm_count,
                    &mut accounts,
                    &lsrm_receipt_keys,
                )?;

                // Collect signers (for account creation).
                for k in 0..lsrm_nft_mint_keys.len() {
                    signers.push(&lsrm_nft_mint_keys[k]);
                    signers.push(&lsrm_nft_token_account_keys[k]);
                }

                // Add the mint_lsrm instruction.
                let mint_lsrm_instr = super::instruction::mint_locked_srm(
                    *self.program(),
                    &accounts,
                    *lsrm_nft_token_account_owner,
                );
                instructions.push(mint_lsrm_instr);

                // Create the tx.
                let (recent_hash, _fee_calc) = self.rpc().get_recent_blockhash()?;
                let tx = Transaction::new_signed_with_payer(
                    &instructions,
                    Some(&self.payer().pubkey()),
                    &signers,
                    recent_hash,
                );
                (tx, lsrm_nft_mint_keys, lsrm_nft_token_account_keys)
            };
            // Execute it.
            self
                .rpc
                .send_and_confirm_transaction_with_spinner_and_config(
                    &tx,
                    self.opts.commitment,
                    self.opts.tx,
                )
                .map_err(|e| ClientError::RpcError(e))
                .map(|sig| {
                    // Format a nice return value.
                    let mut lsrm_nfts = vec![];
                    for _ in 0..lsrm_nft_mint_keys.len() {
                        let mint = lsrm_nft_mint_keys.pop().unwrap();
                        let token_account = lsrm_nft_token_account_keys.pop().unwrap();
                        let receipt = lsrm_receipt_keys.pop().unwrap();
                        lsrm_nfts.push(Lsrm {
                            mint,
                            token_account,
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
        fn create_lsrm_receipts(&self, lsrm_count: usize) -> Result<Vec<Keypair>, ClientError> {
            let mut receipts = vec![];

            for _ in 0..lsrm_count {
                let kp = serum_common::client::rpc::create_account_rent_exempt(
                    self.rpc(),
                    &self.payer(),
                    LsrmReceipt::size().unwrap() as usize,
                    &self.program(),
                ).map_err(|e| ClientError::RawError(e.to_string()))?;

                receipts.push(kp);
            }
            Ok(receipts)
        }

        // Build the create_account instructions for both the SPL NFTs and
        // the safe's mint receipts.
        //
        // Returns the instructions and keys for the created accounts.
        fn create_nfts_instructions_and_keys(
            &self,
            lsrm_count: usize,
            accounts: &mut Vec<AccountMeta>,
            lsrm_receipt_keys: &[Keypair],
        ) -> Result<(Vec<Instruction>, Vec<Keypair>, Vec<Keypair>), ClientError>  {
            let mut lsrm_nft_mint_keys = vec![];
            let mut lsrm_nft_token_account_keys = vec![];

            let mut instructions = vec![];

            let lamports_mint = self.rpc().get_minimum_balance_for_rent_exemption(
                spl_token::state::Mint::LEN,
            )?;
            let lamports_token_account = self.rpc().get_minimum_balance_for_rent_exemption(
                spl_token::state::Account::LEN,
            )?;
            for k in 0..lsrm_count {
                // The NFT Mint to intialize.
                let lsrm_nft_mint = Keypair::generate(&mut OsRng);
                let create_mint_account_instr = solana_sdk::system_instruction::create_account(
                    &self.payer().pubkey(),
                    &lsrm_nft_mint.pubkey(),
                    lamports_mint,
                    spl_token::state::Mint::LEN as u64,
                    &spl_token::ID,
                );

                // The token Account to hold the NFT.
                let lsrm_nft_token_account = Keypair::generate(&mut OsRng);
                let create_token_account_instr = solana_sdk::system_instruction::create_account(
                    &self.payer().pubkey(),
                    &lsrm_nft_token_account.pubkey(),
                    lamports_token_account,
                    spl_token::state::Account::LEN as u64,
                    &spl_token::ID,
                );

                // Push the instructions into the tx.
                instructions.push(create_mint_account_instr);
                instructions.push(create_token_account_instr);

                // Push the accounts for the eventual mint_locked_srm instruction.
                accounts.push(AccountMeta::new(lsrm_nft_mint.pubkey(), true));
                accounts.push(AccountMeta::new(lsrm_nft_token_account.pubkey(), true));
                accounts.push(AccountMeta::new(lsrm_receipt_keys[k].pubkey(), false));

                // Save the keys for return.
                lsrm_nft_mint_keys.push(lsrm_nft_mint);
                lsrm_nft_token_account_keys.push(lsrm_nft_token_account);
            }
            Ok((instructions, lsrm_nft_mint_keys, lsrm_nft_token_account_keys))
        }
    }

    /// Lsrm defines the required keys to redeem and otherwise use lSRM.
    pub struct Lsrm {
        /// The SPL token mint representing the NFT instance.
        pub mint: Keypair,
        /// The only account allowed to own the mint (other than valid
        /// locked programs).
        pub token_account: Keypair,
        /// The receipt account address. Required upon redemption to prove
        /// to the program this is a valid lSRM mint.
        pub receipt: Pubkey,
    }
}
