//! A dummy staking program for testing.

#![cfg_attr(feature = "strict", deny(warnings))]

use instruction::WlInstruction;
use serde::{Deserialize, Serialize};
use serum_common::pack::*;
use solana_client_gen::prelude::*;
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::entrypoint::ProgramResult;
#[cfg(feature = "program")]
use solana_sdk::info;
use solana_sdk::pubkey::Pubkey;

#[cfg(feature = "program")]
solana_sdk::entrypoint!(process_instruction);
#[cfg(feature = "program")]
fn process_instruction(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    info!("process-instruction");

    let instruction: WlInstruction = WlInstruction::unpack(instruction_data).unwrap();

    let result = match instruction {
        WlInstruction::Initialize { nonce } => handlers::initialize(accounts, nonce),
        WlInstruction::Stake { amount } => handlers::stake(accounts, amount),
        WlInstruction::Unstake { amount } => handlers::unstake(accounts, amount),
    };

    result?;

    info!("process-instruction success");

    Ok(())
}

#[cfg(feature = "program")]
mod handlers {
    use super::*;
    pub fn initialize(accounts: &[AccountInfo], nonce: u8) -> ProgramResult {
        info!("handler: initialize");

        let acc_infos = &mut accounts.iter();
        let wl_acc_info = next_account_info(acc_infos)?;

        accounts::Wl::unpack_mut(
            &mut wl_acc_info.try_borrow_mut_data()?,
            &mut |wl: &mut accounts::Wl| {
                wl.nonce = nonce;
                Ok(())
            },
        )
    }

    pub fn stake(accounts: &[AccountInfo], amount: u64) -> ProgramResult {
        info!("handler: stake");
        let acc_infos = &mut accounts.iter();

        let token_acc_info = next_account_info(acc_infos)?;
        let vault_acc_info = next_account_info(acc_infos)?;
        let vault_authority_acc_info = next_account_info(acc_infos)?;
        let token_program_acc_info = next_account_info(acc_infos)?;
        let wl_acc_info = next_account_info(acc_infos)?;

        let wl = accounts::Wl::unpack(&wl_acc_info.try_borrow_data()?)?;
        let nonce = wl.nonce;
        let signer_seeds = accounts::signer_seeds(wl_acc_info.key, &nonce);

        // Delegate transfer to oneself.
        let transfer_instruction = spl_token::instruction::transfer(
            &spl_token::ID,
            token_acc_info.key,
            vault_acc_info.key,
            &vault_authority_acc_info.key,
            &[],
            amount,
        )?;
        solana_sdk::program::invoke_signed(
            &transfer_instruction,
            &[
                vault_acc_info.clone(),
                token_acc_info.clone(),
                vault_authority_acc_info.clone(),
                token_program_acc_info.clone(),
            ],
            &[&signer_seeds],
        )
    }

    pub fn unstake(accounts: &[AccountInfo], amount: u64) -> ProgramResult {
        info!("handler: unstake");
        let acc_infos = &mut accounts.iter();

        let token_acc_info = next_account_info(acc_infos)?;
        let vault_acc_info = next_account_info(acc_infos)?;
        let vault_authority_acc_info = next_account_info(acc_infos)?;
        let token_program_acc_info = next_account_info(acc_infos)?;
        let wl_acc_info = next_account_info(acc_infos)?;

        let wl = accounts::Wl::unpack(&wl_acc_info.try_borrow_data()?)?;
        let nonce = wl.nonce;
        let signer_seeds = accounts::signer_seeds(wl_acc_info.key, &nonce);

        let transfer_instruction = spl_token::instruction::transfer(
            &spl_token::ID,
            vault_acc_info.key,
            token_acc_info.key,
            &vault_authority_acc_info.key,
            &[],
            amount,
        )?;
        solana_sdk::program::invoke_signed(
            &transfer_instruction,
            &[
                vault_acc_info.clone(),
                token_acc_info.clone(),
                vault_authority_acc_info.clone(),
                token_program_acc_info.clone(),
            ],
            &[&signer_seeds],
        )
    }
}

mod accounts {
    use super::*;

    #[cfg(feature = "client")]
    lazy_static::lazy_static! {
        pub static ref WL_SIZE: u64 = Wl::default()
                    .size()
                    .expect("Vesting has a fixed size");
    }

    #[derive(Default, Serialize, Deserialize)]
    pub struct Wl {
        pub nonce: u8,
    }
    serum_common::packable!(Wl);

    pub fn signer_seeds<'a>(wl: &'a Pubkey, nonce: &'a u8) -> [&'a [u8]; 2] {
        [wl.as_ref(), bytemuck::bytes_of(nonce)]
    }
}

#[cfg_attr(feature = "client", solana_client_gen(ext))]
pub mod instruction {
    use super::*;
    #[derive(serde::Serialize, serde::Deserialize)]
    pub enum WlInstruction {
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
        /// 4. `[]`         Wl.
        Stake { amount: u64 },
        /// Accounts:
        ///
        /// 0. `[writable]` Safe vault (to transfer tokens to).
        /// 1. `[writable]` Program token vault.
        /// 2. `[]`         Program vault authority.
        /// 3. `[]`         Token program id.
        /// 4. `[]`         Wl.
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

serum_common::packable!(crate::instruction::WlInstruction);
