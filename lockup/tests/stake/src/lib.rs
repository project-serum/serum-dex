//! Client crate for the test stake program.

use borsh::{BorshDeserialize, BorshSerialize};
use serum_common::pack::*;
use solana_client_gen::prelude::*;

/// Implements the lockup program's whitelist relay interface, allowing it
/// to relay withdrawals and deposits to/from this program.
#[cfg_attr(feature = "client", solana_client_gen(ext))]
pub mod instruction {
    use super::*;
    #[derive(BorshSerialize, BorshDeserialize)]
    pub enum StakeInstruction {
        /// Accounts:
        ///
        /// 0. `[writable]` Whitelist to initialize.
        Initialize { nonce: u8 },
        /// Accounts:
        ///
        /// 0. `[writable]` Safe vault (to transfer tokens from).
        /// 1. `[writable]` Program token vault.
        /// 2. `[]`         Program vault authority.
        /// 3. `[]`         Token program id.
        /// 4. `[]`         Instance.
        Stake { amount: u64 },
        /// Accounts:
        ///
        /// 0. `[writable]` Safe vault (to transfer tokens to).
        /// 1. `[writable]` Program token vault.
        /// 2. `[]`         Program vault authority.
        /// 3. `[]`         Token program id.
        /// 4. `[]`         Instance.
        Unstake { amount: u64 },
    }
}

#[cfg(feature = "client")]
solana_client_gen_extension! {
    impl Client {
        pub fn init(&self, mint: &Pubkey) -> Result<InitializeResponse, ClientError> {
            let wl_acc = Keypair::generate(&mut OsRng);
            let (vault_authority, nonce) = Pubkey::find_program_address(
                &[wl_acc.pubkey().as_ref()],
                self.program(),
            );

            let vault = serum_common::client::rpc::create_token_account(
                self.rpc(),
                mint,
                &vault_authority,
                self.payer(),
            ).map_err(|e| ClientError::RawError(e.to_string()))?;


            let lamports = self
                .rpc()
                .get_minimum_balance_for_rent_exemption(
                    *crate::accounts::WL_SIZE as usize,
                )
                .map_err(ClientError::RpcError)?;

            let create_acc_instr = system_instruction::create_account(
                &self.payer().pubkey(),
                &wl_acc.pubkey(),
                lamports,
                *crate::accounts::WL_SIZE,
                self.program(),
            );

            let initialize_instr = super::instruction::initialize(
                *self.program(),
                &[AccountMeta::new(wl_acc.pubkey(), false)],
                nonce,
            );

            let instructions = vec![create_acc_instr, initialize_instr];

            let tx = {
                let (recent_hash, _fee_calc) = self
                    .rpc()
                    .get_recent_blockhash()
                    .map_err(|e| ClientError::RawError(e.to_string()))?;
                let signers = vec![self.payer(), &wl_acc, &wl_acc];
                Transaction::new_signed_with_payer(
                    &instructions,
                    Some(&self.payer().pubkey()),
                    &signers,
                    recent_hash,
                )
            };
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
                    vault_authority,
                    vault: vault.pubkey(),
                    instance: wl_acc.pubkey(),
                    nonce,
                })
        }
    }
    pub struct InitializeResponse {
        pub signature: solana_sdk::signature::Signature,
        pub nonce: u8,
        pub instance: Pubkey,
        pub vault: Pubkey,
        pub vault_authority: Pubkey,
    }
}

serum_common::packable!(crate::instruction::StakeInstruction);

pub mod accounts {
    use super::*;

    #[cfg(feature = "client")]
    lazy_static::lazy_static! {
        pub static ref WL_SIZE: u64 = Instance::default()
                    .size()
                    .expect("Vesting has a fixed size");
    }

    #[derive(Default, BorshSerialize, BorshDeserialize)]
    pub struct Instance {
        pub nonce: u8,
    }
    serum_common::packable!(Instance);

    pub fn signer_seeds<'a>(wl: &'a Pubkey, nonce: &'a u8) -> [&'a [u8]; 2] {
        [wl.as_ref(), bytemuck::bytes_of(nonce)]
    }
}
