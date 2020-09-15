//! Program state processor

#![cfg(feature = "program")]

use crate::{
    error::EtfError,
    instruction::EtfInstruction,
    state::{Invariant, EtfInfo},
};
use num_traits::FromPrimitive;
#[cfg(not(target_arch = "bpf"))]
use solana_sdk::instruction::Instruction;
#[cfg(target_arch = "bpf")]
use solana_sdk::program::invoke_signed;
use solana_sdk::{
    account_info::{next_account_info, AccountInfo},
    decode_error::DecodeError,
    entrypoint::ProgramResult,
    info,
    program_error::PrintProgramError,
    program_error::ProgramError,
    pubkey::Pubkey,
};
use spl_token::pack::Pack;

// Test program id for the etf program.
#[cfg(not(target_arch = "bpf"))]
const ETF_PROGRAM_ID: Pubkey = Pubkey::new_from_array([2u8; 32]);
// Test program id for the token program.
#[cfg(not(target_arch = "bpf"))]
const TOKEN_PROGRAM_ID: Pubkey = Pubkey::new_from_array([1u8; 32]);

/// Program state handler.
pub struct Processor {}
impl Processor {
    /// Unpacks a spl_token `Account`.
    pub fn unpack_token_account(data: &[u8]) -> Result<spl_token::state::Account, EtfError> {
        spl_token::state::Account::unpack(data).map_err(|_| EtfError::ExpectedAccount)
    }

    /// Unpacks a spl_token `Mint`.
    pub fn unpack_mint(data: &[u8]) -> Result<spl_token::state::Mint, EtfError> {
        spl_token::state::Mint::unpack(data).map_err(|_| EtfError::ExpectedAccount)
    }

    /// Calculates the authority id by generating a program address.
    pub fn authority_id(
        program_id: &Pubkey,
        my_info: &Pubkey,
        nonce: u8,
    ) -> Result<Pubkey, EtfError> {
        Pubkey::create_program_address(&[&my_info.to_bytes()[..32], &[nonce]], program_id)
            .or(Err(EtfError::InvalidProgramAddress))
    }

    /// Issue a spl_token `Burn` instruction.
    pub fn token_burn<'a>(
        etf: &Pubkey,
        token_program: AccountInfo<'a>,
        burn_account: AccountInfo<'a>,
        mint: AccountInfo<'a>,
        authority: AccountInfo<'a>,
        nonce: u8,
        amount: u64,
    ) -> Result<(), ProgramError> {
        let etf_bytes = etf.to_bytes();
        let authority_signature_seeds = [&etf_bytes[..32], &[nonce]];
        let signers = &[&authority_signature_seeds[..]];

        let ix = spl_token::instruction::burn(
            token_program.key,
            burn_account.key,
            mint.key,
            authority.key,
            &[],
            amount,
        )?;

        invoke_signed(
            &ix,
            &[burn_account, mint, authority, token_program],
            signers,
        )
    }

    /// Issue a spl_token `MintTo` instruction.
    pub fn token_mint_to<'a>(
        etf: &Pubkey,
        token_program: AccountInfo<'a>,
        mint: AccountInfo<'a>,
        destination: AccountInfo<'a>,
        authority: AccountInfo<'a>,
        nonce: u8,
        amount: u64,
    ) -> Result<(), ProgramError> {
        let etf_bytes = etf.to_bytes();
        let authority_signature_seeds = [&etf_bytes[..32], &[nonce]];
        let signers = &[&authority_signature_seeds[..]];
        let ix = spl_token::instruction::mint_to(
            token_program.key,
            mint.key,
            destination.key,
            authority.key,
            &[],
            amount,
        )?;

        invoke_signed(&ix, &[mint, destination, authority, token_program], signers)
    }

    /// Issue a spl_token `Transfer` instruction.
    pub fn token_transfer<'a>(
        etf: &Pubkey,
        token_program: AccountInfo<'a>,
        source: AccountInfo<'a>,
        destination: AccountInfo<'a>,
        authority: AccountInfo<'a>,
        nonce: u8,
        amount: u64,
    ) -> Result<(), ProgramError> {
        let etf_bytes = etf.to_bytes();
        let authority_signature_seeds = [&etf_bytes[..32], &[nonce]];
        let signers = &[&authority_signature_seeds[..]];
        let ix = spl_token::instruction::transfer(
            token_program.key,
            source.key,
            destination.key,
            authority.key,
            &[],
            amount,
        )?;
        invoke_signed(
            &ix,
            &[source, destination, authority, token_program],
            signers,
        )
    }

    /// Processes an [Initialize](enum.Instruction.html).
    pub fn process_initialize(
        program_id: &Pubkey,
        nonce: u8,
        accounts: &[AccountInfo],
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let etf_info = next_account_info(account_info_iter)?;
        let authority_info = next_account_info(account_info_iter)?;
        let token_a_info = next_account_info(account_info_iter)?;
        let token_b_info = next_account_info(account_info_iter)?;
        let pool_info = next_account_info(account_info_iter)?;
        let user_destination_info = next_account_info(account_info_iter)?;
        let token_program_info = next_account_info(account_info_iter)?;

        let token_etf = EtfInfo::unpack_unchecked(&etf_info.data.borrow())?;
        if token_etf.is_initialized {
            return Err(EtfError::AlreadyInUse.into());
        }

        if *authority_info.key != Self::authority_id(program_id, etf_info.key, nonce)? {
            return Err(EtfError::InvalidProgramAddress.into());
        }
        let token_a = Self::unpack_token_account(&token_a_info.data.borrow())?;
        let token_b = Self::unpack_token_account(&token_b_info.data.borrow())?;
        let pool_mint = Self::unpack_mint(&pool_info.data.borrow())?;
        if *authority_info.key != token_a.owner {
            return Err(EtfError::InvalidOwner.into());
        }
        if *authority_info.key != token_b.owner {
            return Err(EtfError::InvalidOwner.into());
        }
        if spl_token::option::COption::Some(*authority_info.key) != pool_mint.mint_authority {
            return Err(EtfError::InvalidOwner.into());
        }
        if token_b.amount == 0 {
            return Err(EtfError::InvalidSupply.into());
        }
        if token_a.amount == 0 {
            return Err(EtfError::InvalidSupply.into());
        }
        if token_a.delegate.is_some() {
            return Err(EtfError::InvalidDelegate.into());
        }
        if token_b.delegate.is_some() {
            return Err(EtfError::InvalidDelegate.into());
        }

        // liquidity is measured in terms of token_a's value since both sides of
        // the pool are equal
        let amount = token_a.amount;
        Self::token_mint_to(
            etf_info.key,
            token_program_info.clone(),
            pool_info.clone(),
            user_destination_info.clone(),
            authority_info.clone(),
            nonce,
            amount,
        )?;

        let obj = EtfInfo {
            is_initialized: true,
            nonce,
            token_a: *token_a_info.key,
            token_b: *token_b_info.key,
            pool_mint: *pool_info.key,
        };
        obj.pack(&mut etf_info.data.borrow_mut());
        Ok(())
    }

    /// Processes an [Deposit](enum.Instruction.html).
    pub fn process_deposit(
        program_id: &Pubkey,
        a_amount: u64,
        accounts: &[AccountInfo],
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let etf_info = next_account_info(account_info_iter)?;
        let authority_info = next_account_info(account_info_iter)?;
        let source_a_info = next_account_info(account_info_iter)?;
        let source_b_info = next_account_info(account_info_iter)?;
        let token_a_info = next_account_info(account_info_iter)?;
        let token_b_info = next_account_info(account_info_iter)?;
        let pool_info = next_account_info(account_info_iter)?;
        let dest_info = next_account_info(account_info_iter)?;
        let token_program_info = next_account_info(account_info_iter)?;

        let token_etf = EtfInfo::unpack(&etf_info.data.borrow())?;
        if *authority_info.key != Self::authority_id(program_id, etf_info.key, token_etf.nonce)? {
            return Err(EtfError::InvalidProgramAddress.into());
        }
        if *token_a_info.key != token_etf.token_a {
            return Err(EtfError::InvalidInput.into());
        }
        if *token_b_info.key != token_etf.token_b {
            return Err(EtfError::InvalidInput.into());
        }
        if *pool_info.key != token_etf.pool_mint {
            return Err(EtfError::InvalidInput.into());
        }
        let token_a = Self::unpack_token_account(&token_a_info.data.borrow())?;
        let token_b = Self::unpack_token_account(&token_b_info.data.borrow())?;

        let invariant = Invariant {
            token_a: token_a.amount,
            token_b: token_b.amount,
        };
        let b_amount = invariant
            .exchange_rate(a_amount)
            .ok_or(EtfError::CalculationFailure)?;

        // liquidity is measured in terms of token_a's value
        // since both sides of the pool are equal
        let output = a_amount;

        Self::token_transfer(
            etf_info.key,
            token_program_info.clone(),
            source_a_info.clone(),
            token_a_info.clone(),
            authority_info.clone(),
            token_etf.nonce,
            a_amount,
        )?;
        Self::token_transfer(
            etf_info.key,
            token_program_info.clone(),
            source_b_info.clone(),
            token_b_info.clone(),
            authority_info.clone(),
            token_etf.nonce,
            b_amount,
        )?;
        Self::token_mint_to(
            etf_info.key,
            token_program_info.clone(),
            pool_info.clone(),
            dest_info.clone(),
            authority_info.clone(),
            token_etf.nonce,
            output,
        )?;

        Ok(())
    }

    /// Processes an [Withdraw](enum.Instruction.html).
    pub fn process_withdraw(
        program_id: &Pubkey,
        amount: u64,
        accounts: &[AccountInfo],
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let etf_info = next_account_info(account_info_iter)?;
        let authority_info = next_account_info(account_info_iter)?;
        let pool_mint_info = next_account_info(account_info_iter)?;
        let source_info = next_account_info(account_info_iter)?;
        let token_a_info = next_account_info(account_info_iter)?;
        let token_b_info = next_account_info(account_info_iter)?;
        let dest_token_a_info = next_account_info(account_info_iter)?;
        let dest_token_b_info = next_account_info(account_info_iter)?;
        let token_program_info = next_account_info(account_info_iter)?;

        let token_etf = EtfInfo::unpack(&etf_info.data.borrow())?;
        if *authority_info.key != Self::authority_id(program_id, etf_info.key, token_etf.nonce)? {
            return Err(EtfError::InvalidProgramAddress.into());
        }
        if *token_a_info.key != token_etf.token_a {
            return Err(EtfError::InvalidInput.into());
        }
        if *token_b_info.key != token_etf.token_b {
            return Err(EtfError::InvalidInput.into());
        }

        let token_a = Self::unpack_token_account(&token_a_info.data.borrow())?;
        let token_b = Self::unpack_token_account(&token_b_info.data.borrow())?;

        let invariant = Invariant {
            token_a: token_a.amount,
            token_b: token_b.amount,
        };

        let a_amount = amount;
        let b_amount = invariant
            .exchange_rate(a_amount)
            .ok_or(EtfError::CalculationFailure)?;

        Self::token_transfer(
            etf_info.key,
            token_program_info.clone(),
            token_a_info.clone(),
            dest_token_a_info.clone(),
            authority_info.clone(),
            token_etf.nonce,
            a_amount,
        )?;
        Self::token_transfer(
            etf_info.key,
            token_program_info.clone(),
            token_b_info.clone(),
            dest_token_b_info.clone(),
            authority_info.clone(),
            token_etf.nonce,
            b_amount,
        )?;
        Self::token_burn(
            etf_info.key,
            token_program_info.clone(),
            source_info.clone(),
            pool_mint_info.clone(),
            authority_info.clone(),
            token_etf.nonce,
            amount,
        )?;
        Ok(())
    }

    /// Processes an [Instruction](enum.Instruction.html).
    pub fn process(program_id: &Pubkey, accounts: &[AccountInfo], input: &[u8]) -> ProgramResult {
        let instruction = EtfInstruction::unpack(input)?;
        match instruction {
            EtfInstruction::Initialize {
                nonce,
            } => {
                info!("Instruction: Init");
                Self::process_initialize(
                    program_id,
                    nonce,
                    accounts,
                )
            }
            EtfInstruction::Deposit { amount } => {
                info!("Instruction: Deposit");
                Self::process_deposit(program_id, amount, accounts)
            }
            EtfInstruction::Withdraw { amount } => {
                info!("Instruction: Withdraw");
                Self::process_withdraw(program_id, amount, accounts)
            }
        }
    }
}

/// Routes invokes to the token program, used for testing.
#[cfg(not(target_arch = "bpf"))]
pub fn invoke_signed<'a>(
    instruction: &Instruction,
    account_infos: &[AccountInfo<'a>],
    signers_seeds: &[&[&[u8]]],
) -> ProgramResult {
    let mut new_account_infos = vec![];

    // mimic check for token program in accounts
    if !account_infos.iter().any(|x| *x.key == TOKEN_PROGRAM_ID) {
        return Err(ProgramError::InvalidAccountData);
    }

    for meta in instruction.accounts.iter() {
        for account_info in account_infos.iter() {
            if meta.pubkey == *account_info.key {
                let mut new_account_info = account_info.clone();
                for seeds in signers_seeds.iter() {
                    let signer = Pubkey::create_program_address(&seeds, &ETF_PROGRAM_ID).unwrap();
                    if *account_info.key == signer {
                        new_account_info.is_signer = true;
                    }
                }
                new_account_infos.push(new_account_info);
            }
        }
    }

    spl_token::processor::Processor::process(
        &instruction.program_id,
        &new_account_infos,
        &instruction.data,
    )
}

impl PrintProgramError for EtfError {
    fn print<E>(&self)
    where
        E: 'static + std::error::Error + DecodeError<E> + PrintProgramError + FromPrimitive,
    {
        match self {
            EtfError::AlreadyInUse => info!("Error: AlreadyInUse"),
            EtfError::InvalidProgramAddress => info!("Error: InvalidProgramAddress"),
            EtfError::InvalidOwner => info!("Error: InvalidOwner"),
            EtfError::ExpectedToken => info!("Error: ExpectedToken"),
            EtfError::ExpectedAccount => info!("Error: ExpectedAccount"),
            EtfError::InvalidSupply => info!("Error: InvalidSupply"),
            EtfError::InvalidDelegate => info!("Error: InvalidDelegate"),
            EtfError::InvalidState => info!("Error: InvalidState"),
            EtfError::InvalidInput => info!("Error: InvalidInput"),
            EtfError::InvalidOutput => info!("Error: InvalidOutput"),
            EtfError::CalculationFailure => info!("Error: CalculationFailure"),
            EtfError::InvalidInstruction => info!("Error: InvalidInstruction"),
        }
    }
}

// Pull in syscall stubs when building for non-BPF targets
#[cfg(not(target_arch = "bpf"))]
solana_sdk::program_stubs!();

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        instruction::{deposit, initialize, withdraw},
    };
    use solana_sdk::{
        account::Account, account_info::create_is_signer_account_infos, instruction::Instruction,
        rent::Rent, sysvar::rent,
    };
    use spl_token::{
        instruction::{initialize_account, initialize_mint, mint_to},
        pack::Pack,
        processor::Processor as SplProcessor,
        state::{Account as SplAccount, Mint as SplMint},
    };
    use std::mem::size_of;

    struct EtfAccountInfo {
        nonce: u8,
        etf_key: Pubkey,
        etf_account: Account,
        pool_mint_key: Pubkey,
        pool_mint_account: Account,
        pool_token_key: Pubkey,
        pool_token_account: Account,
        token_a_key: Pubkey,
        token_a_account: Account,
        token_a_mint_key: Pubkey,
        token_a_mint_account: Account,
        token_b_key: Pubkey,
        token_b_account: Account,
        token_b_mint_key: Pubkey,
        token_b_mint_account: Account,
    }

    fn mint_minimum_balance() -> u64 {
        Rent::default().minimum_balance(SplMint::get_packed_len())
    }

    fn account_minimum_balance() -> u64 {
        Rent::default().minimum_balance(SplAccount::get_packed_len())
    }

    fn pubkey_rand() -> Pubkey {
        Pubkey::new(&rand::random::<[u8; 32]>())
    }

    fn do_process_instruction(
        instruction: Instruction,
        accounts: Vec<&mut Account>,
    ) -> ProgramResult {
        let mut meta = instruction
            .accounts
            .iter()
            .zip(accounts)
            .map(|(account_meta, account)| (&account_meta.pubkey, account_meta.is_signer, account))
            .collect::<Vec<_>>();

        let account_infos = create_is_signer_account_infos(&mut meta);
        if instruction.program_id == ETF_PROGRAM_ID {
            Processor::process(&instruction.program_id, &account_infos, &instruction.data)
        } else {
            SplProcessor::process(&instruction.program_id, &account_infos, &instruction.data)
        }
    }

    fn mint_token(
        program_id: &Pubkey,
        mint_key: &Pubkey,
        mut mint_account: &mut Account,
        authority_key: &Pubkey,
        amount: u64,
    ) -> (Pubkey, Account) {
        let account_key = pubkey_rand();
        let mut account_account = Account::new(
            account_minimum_balance(),
            SplAccount::get_packed_len(),
            &program_id,
        );
        let mut authority_account = Account::default();
        let mut rent_sysvar_account = rent::create_account(1, &Rent::free());

        // create account
        do_process_instruction(
            initialize_account(&program_id, &account_key, &mint_key, authority_key).unwrap(),
            vec![
                &mut account_account,
                &mut mint_account,
                &mut authority_account,
                &mut rent_sysvar_account,
            ],
        )
        .unwrap();

        do_process_instruction(
            mint_to(
                &program_id,
                &mint_key,
                &account_key,
                &authority_key,
                &[],
                amount,
            )
            .unwrap(),
            vec![
                &mut mint_account,
                &mut account_account,
                &mut authority_account,
            ],
        )
        .unwrap();

        (account_key, account_account)
    }

    fn create_mint(program_id: &Pubkey, authority_key: &Pubkey) -> (Pubkey, Account) {
        let mint_key = pubkey_rand();
        let mut mint_account = Account::new(
            mint_minimum_balance(),
            SplMint::get_packed_len(),
            &program_id,
        );
        let mut rent_sysvar_account = rent::create_account(1, &Rent::free());

        // create token mint
        do_process_instruction(
            initialize_mint(&program_id, &mint_key, authority_key, None, 2).unwrap(),
            vec![&mut mint_account, &mut rent_sysvar_account],
        )
        .unwrap();

        (mint_key, mint_account)
    }

    #[test]
    fn test_token_program_id_error() {
        let etf_key = pubkey_rand();
        let mut mint = (pubkey_rand(), Account::default());
        let mut destination = (pubkey_rand(), Account::default());
        let token_program = (TOKEN_PROGRAM_ID, Account::default());
        let (authority_key, nonce) =
            Pubkey::find_program_address(&[&etf_key.to_bytes()[..]], &ETF_PROGRAM_ID);
        let mut authority = (authority_key, Account::default());
        let etf_bytes = etf_key.to_bytes();
        let authority_signature_seeds = [&etf_bytes[..32], &[nonce]];
        let signers = &[&authority_signature_seeds[..]];
        let ix = spl_token::instruction::mint_to(
            &token_program.0,
            &mint.0,
            &destination.0,
            &authority.0,
            &[],
            10,
        )
        .unwrap();
        let mint = (&mut mint).into();
        let destination = (&mut destination).into();
        let authority = (&mut authority).into();

        let err = invoke_signed(&ix, &[mint, destination, authority], signers).unwrap_err();
        assert_eq!(err, ProgramError::InvalidAccountData);
    }

    fn initialize_etf<'a>(
        token_a_amount: u64,
        token_b_amount: u64,
    ) -> EtfAccountInfo {
        let etf_key = pubkey_rand();
        let mut etf_account = Account::new(0, size_of::<EtfInfo>(), &ETF_PROGRAM_ID);
        let (authority_key, nonce) =
            Pubkey::find_program_address(&[&etf_key.to_bytes()[..]], &ETF_PROGRAM_ID);

        let (pool_mint_key, mut pool_mint_account) = create_mint(&TOKEN_PROGRAM_ID, &authority_key);
        let (pool_token_key, mut pool_token_account) = mint_token(
            &TOKEN_PROGRAM_ID,
            &pool_mint_key,
            &mut pool_mint_account,
            &authority_key,
            0,
        );
        let (token_a_mint_key, mut token_a_mint_account) =
            create_mint(&TOKEN_PROGRAM_ID, &authority_key);
        let (token_a_key, mut token_a_account) = mint_token(
            &TOKEN_PROGRAM_ID,
            &token_a_mint_key,
            &mut token_a_mint_account,
            &authority_key,
            token_a_amount,
        );
        let (token_b_mint_key, mut token_b_mint_account) =
            create_mint(&TOKEN_PROGRAM_ID, &authority_key);
        let (token_b_key, mut token_b_account) = mint_token(
            &TOKEN_PROGRAM_ID,
            &token_b_mint_key,
            &mut token_b_mint_account,
            &authority_key,
            token_b_amount,
        );

        let mut authority_account = Account::default();
        do_process_instruction(
            initialize(
                &ETF_PROGRAM_ID,
                &TOKEN_PROGRAM_ID,
                &etf_key,
                &authority_key,
                &token_a_key,
                &token_b_key,
                &pool_mint_key,
                &pool_token_key,
                nonce,
            )
            .unwrap(),
            vec![
                &mut etf_account,
                &mut authority_account,
                &mut token_a_account,
                &mut token_b_account,
                &mut pool_mint_account,
                &mut pool_token_account,
                &mut Account::default(),
            ],
        )
        .unwrap();
        EtfAccountInfo {
            nonce,
            etf_key,
            etf_account,
            pool_mint_key,
            pool_mint_account,
            pool_token_key,
            pool_token_account,
            token_a_key,
            token_a_account,
            token_a_mint_key,
            token_a_mint_account,
            token_b_key,
            token_b_account,
            token_b_mint_key,
            token_b_mint_account,
        }
    }

    #[test]
    fn test_initialize() {
        let token_a_amount = 1000;
        let token_b_amount = 2000;
        let etf_accounts = initialize_etf(
            token_a_amount,
            token_b_amount,
        );
        let etf_info = EtfInfo::unpack(&etf_accounts.etf_account.data).unwrap();
        assert_eq!(etf_info.is_initialized, true);
        assert_eq!(etf_info.nonce, etf_accounts.nonce);
        assert_eq!(etf_info.token_a, etf_accounts.token_a_key);
        assert_eq!(etf_info.token_b, etf_accounts.token_b_key);
        assert_eq!(etf_info.pool_mint, etf_accounts.pool_mint_key);
        let token_a = Processor::unpack_token_account(&etf_accounts.token_a_account.data).unwrap();
        assert_eq!(token_a.amount, token_a_amount);
        let token_b = Processor::unpack_token_account(&etf_accounts.token_b_account.data).unwrap();
        assert_eq!(token_b.amount, token_b_amount);
        let pool_account =
            Processor::unpack_token_account(&etf_accounts.pool_token_account.data).unwrap();
        let pool_mint = Processor::unpack_mint(&etf_accounts.pool_mint_account.data).unwrap();
        assert_eq!(pool_mint.supply, pool_account.amount);
    }

    #[test]
    fn test_deposit() {
        let token_a_amount = 1000;
        let token_b_amount = 8000;
        let mut accounts = initialize_etf(
            token_a_amount,
            token_b_amount,
        );
        let seeds = [&accounts.etf_key.to_bytes()[..32], &[accounts.nonce]];
        let authority_key = Pubkey::create_program_address(&seeds, &ETF_PROGRAM_ID).unwrap();
        let deposit_a = token_a_amount / 10;
        let (depositor_token_a_key, mut depositor_token_a_account) = mint_token(
            &TOKEN_PROGRAM_ID,
            &accounts.token_a_mint_key,
            &mut accounts.token_a_mint_account,
            &authority_key,
            deposit_a,
        );
        let deposit_b = token_b_amount / 10;
        let (depositor_token_b_key, mut depositor_token_b_account) = mint_token(
            &TOKEN_PROGRAM_ID,
            &accounts.token_b_mint_key,
            &mut accounts.token_b_mint_account,
            &authority_key,
            deposit_b,
        );
        let initial_pool = 10;
        let (depositor_pool_key, mut depositor_pool_account) = mint_token(
            &TOKEN_PROGRAM_ID,
            &accounts.pool_mint_key,
            &mut accounts.pool_mint_account,
            &authority_key,
            initial_pool,
        );

        do_process_instruction(
            deposit(
                &ETF_PROGRAM_ID,
                &TOKEN_PROGRAM_ID,
                &accounts.etf_key,
                &authority_key,
                &depositor_token_a_key,
                &depositor_token_b_key,
                &accounts.token_a_key,
                &accounts.token_b_key,
                &accounts.pool_mint_key,
                &depositor_pool_key,
                deposit_a,
            )
            .unwrap(),
            vec![
                &mut accounts.etf_account,
                &mut Account::default(),
                &mut depositor_token_a_account,
                &mut depositor_token_b_account,
                &mut accounts.token_a_account,
                &mut accounts.token_b_account,
                &mut accounts.pool_mint_account,
                &mut depositor_pool_account,
                &mut Account::default(),
            ],
        )
        .unwrap();
        let token_a = Processor::unpack_token_account(&accounts.token_a_account.data).unwrap();
        assert_eq!(token_a.amount, deposit_a + token_a_amount);
        let token_b = Processor::unpack_token_account(&accounts.token_b_account.data).unwrap();
        assert_eq!(token_b.amount, deposit_b + token_b_amount);
        let depositor_token_a =
            Processor::unpack_token_account(&depositor_token_a_account.data).unwrap();
        assert_eq!(depositor_token_a.amount, 0);
        let depositor_token_b =
            Processor::unpack_token_account(&depositor_token_b_account.data).unwrap();
        assert_eq!(depositor_token_b.amount, 0);
        let depositor_pool_account =
            Processor::unpack_token_account(&depositor_pool_account.data).unwrap();
        let pool_account =
            Processor::unpack_token_account(&accounts.pool_token_account.data).unwrap();
        let pool_mint = Processor::unpack_mint(&accounts.pool_mint_account.data).unwrap();
        assert_eq!(
            pool_mint.supply,
            pool_account.amount + depositor_pool_account.amount
        );
    }

    #[test]
    fn test_withdraw() {
        let token_a_amount = 1000;
        let token_b_amount = 2000;
        let mut accounts = initialize_etf(
            token_a_amount,
            token_b_amount,
        );
        let seeds = [&accounts.etf_key.to_bytes()[..32], &[accounts.nonce]];
        let authority_key = Pubkey::create_program_address(&seeds, &ETF_PROGRAM_ID).unwrap();
        let initial_a = token_a_amount / 10;
        let (withdraw_token_a_key, mut withdraw_token_a_account) = mint_token(
            &TOKEN_PROGRAM_ID,
            &accounts.token_a_mint_key,
            &mut accounts.token_a_mint_account,
            &authority_key,
            initial_a,
        );
        let initial_b = token_b_amount / 10;
        let (withdraw_token_b_key, mut withdraw_token_b_account) = mint_token(
            &TOKEN_PROGRAM_ID,
            &accounts.token_b_mint_key,
            &mut accounts.token_b_mint_account,
            &authority_key,
            initial_b,
        );

        let withdraw_amount = token_a_amount / 4;
        do_process_instruction(
            withdraw(
                &ETF_PROGRAM_ID,
                &TOKEN_PROGRAM_ID,
                &accounts.etf_key,
                &authority_key,
                &accounts.pool_mint_key,
                &accounts.pool_token_key,
                &accounts.token_a_key,
                &accounts.token_b_key,
                &withdraw_token_a_key,
                &withdraw_token_b_key,
                withdraw_amount,
            )
            .unwrap(),
            vec![
                &mut accounts.etf_account,
                &mut Account::default(),
                &mut accounts.pool_mint_account,
                &mut accounts.pool_token_account,
                &mut accounts.token_a_account,
                &mut accounts.token_b_account,
                &mut withdraw_token_a_account,
                &mut withdraw_token_b_account,
                &mut Account::default(),
            ],
        )
        .unwrap();

        let token_a = Processor::unpack_token_account(&accounts.token_a_account.data).unwrap();
        assert_eq!(token_a.amount, token_a_amount - withdraw_amount);
        let token_b = Processor::unpack_token_account(&accounts.token_b_account.data).unwrap();
        assert_eq!(token_b.amount, token_b_amount - (withdraw_amount * 2));
        let withdraw_token_a =
            Processor::unpack_token_account(&withdraw_token_a_account.data).unwrap();
        assert_eq!(withdraw_token_a.amount, initial_a + withdraw_amount);
        let withdraw_token_b =
            Processor::unpack_token_account(&withdraw_token_b_account.data).unwrap();
        assert_eq!(withdraw_token_b.amount, initial_b + (withdraw_amount * 2));
        let pool_account =
            Processor::unpack_token_account(&accounts.pool_token_account.data).unwrap();
        let pool_mint = Processor::unpack_mint(&accounts.pool_mint_account.data).unwrap();
        assert_eq!(pool_mint.supply, pool_account.amount);
    }
}
