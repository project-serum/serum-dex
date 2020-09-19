use crate::accounts::{LsrmReceipt, SafeAccount, VestingAccount};
use serde::{Deserialize, Serialize};
use solana_client_gen::prelude::*;
use solana_client_gen::prelude::*;
use solana_client_gen::solana_sdk;
use solana_client_gen::solana_sdk::instruction::AccountMeta;
use solana_client_gen::solana_sdk::pubkey::Pubkey;
use spl_token::pack::Pack;

// Add custom apis (in addition to those generated).
#[cfg(feature = "client")]
solana_client_gen_extension! {
    use solana_client_gen::solana_sdk::signers::Signers;

    impl Client {
        /// Creates a multi instruction transaction. For each `lsrm_count` number
        /// of lSRM NFTs to mint, executes
        ///
        /// * system::create_account for the lSRM NFT SPL mint
        /// * system::create_account for the safe's lSRM receipt
        ///
        /// And after executing those for all NFTs, finally executes the
        /// MintLockedSrm instruction the Safe program.
        ///
        /// The motivation here is that, given a coin mint, the program needs to
        /// know if it should acknowledge the mint by printing a new receipt.
        ///
        /// It can do this by either having a mapping in storage and checking
        /// it when the mint instruction is run, or, it can *only* print receipts
        /// for NFT mints that are not yet initialized (we choose the latter).
        /// However, inorder to safely initialize a mint, we must have the mint's
        /// create account instruction run in the same transaction as the initialize
        /// mint instruction.
        ///
        /// The lSRM receipt instructions are added in for convenience.
        pub fn create_accounts_and_mint_locked_srm_with_signers<T: Signers>(
            &self,
            lsrm_count: usize,
            signers: T,
            mut accounts: Vec<AccountMeta>,
        ) -> Result<(Signature, Vec<Keypair>), ClientError>{
            // Build the create_account instructions for both the SPL NFTs and
            // the safe's mint receipts.
            let (mut instructions, lsrm_nft_mint_keys) = {
                let mut lsrm_nft_mint_keys = vec![];
                let mut instructions = vec![];

                let lamports = self.rpc().get_minimum_balance_for_rent_exemption(
                    spl_token::state::Mint::LEN,
                )?;

                for _ in 0..lsrm_count {
                    // The NFT to intialize.
                    let lsrm_nft_mint = Keypair::generate(&mut OsRng);
                    let lsrm_nft_mint_authority = self.program();
                    let create_mint_account_instr = solana_sdk::system_instruction::create_account(
                        &self.payer().pubkey(),
                        &lsrm_nft_mint.pubkey(),
                        lamports,
                        spl_token::state::Mint::LEN as u64,
                        &spl_token::ID,
                    );

                    // The receipt for the mint								.
                    let lsrm_receipt = Keypair::generate(&mut OsRng);
                    let lamports = self.rpc().get_minimum_balance_for_rent_exemption(
                        LsrmReceipt::SIZE,
                    )?;
                    let lsrm_receipt_instr = solana_sdk::system_instruction::create_account(
                        &self.payer().pubkey(),
                        &lsrm_receipt.pubkey(),
                        lamports,
                        LsrmReceipt::SIZE as u64,
                        self.program(),
                    );

                    // Push the isntructions into the tx.
                    instructions.push(create_mint_account_instr);
                    instructions.push(lsrm_receipt_instr);

                    // Push the accounts for the eventual mint_locked_srm instruction.
                    accounts.push(AccountMeta::new(lsrm_nft_mint.pubkey(), false));
                    accounts.push(AccountMeta::new(lsrm_receipt.pubkey(), false));

                    // Save the NFT keys for return.
                    lsrm_nft_mint_keys.push(lsrm_nft_mint);
                }
                (instructions, lsrm_nft_mint_keys)
            };

            // The final MintLockedSrm instruction on the safe.
            let mint_lsrm_instr = super::instruction::mint_locked_srm(*self.program(), &accounts);
            instructions.push(mint_lsrm_instr);

            // Lastly, execute the transaction.
            let (recent_hash, _fee_calc) = self.rpc().get_recent_blockhash()?;
            let tx = Transaction::new_signed_with_payer(
                &instructions,
                Some(&self.payer().pubkey()),
                &signers,
                recent_hash,
            );
            self
                .rpc
                .send_and_confirm_transaction_with_spinner_and_config(
                    &tx,
                    self.opts.commitment,
                    self.opts.tx,
                )
                .map_err(|e| ClientError::RpcError(e))
                .map(|sig| (sig, lsrm_nft_mint_keys))
        }
    }
}
