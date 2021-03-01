#![deny(safe_packed_borrows)]
use std::mem::size_of;

use bumpalo::Bump;
use safe_transmute::to_bytes::{transmute_to_bytes, transmute_to_bytes_mut};
use solana_program::account_info::AccountInfo;
use solana_program::bpf_loader;
use solana_program::clock::Epoch;
use solana_program::program_pack::Pack;
use solana_program::instruction::Instruction;
use solana_program::entrypoint::ProgramResult;
use solana_program::pubkey::Pubkey;
use solana_program::rent::Rent;
use solana_program::system_program;
use solana_program::sysvar;
use solana_program::sysvar::Sysvar;
use spl_token::state::Account as SplAccount;
use spl_token::state::Mint;

use serum_dex::error::DexResult;
use serum_dex::instruction::{fee_sweeper, initialize_market};
use serum_dex::state::{
    gen_vault_signer_key, strip_header, EventQueue, MarketState, Queue, RequestQueue, State,
};

fn random_pubkey(bump: &Bump) -> &Pubkey {
    bump.alloc(Pubkey::new(transmute_to_bytes(&rand::random::<[u64; 4]>())))
}

fn allocate_dex_owned_account(unpadded_size: usize, bump: &Bump) -> &mut [u8] {
    assert_eq!(unpadded_size % 8, 0);
    let padded_size = unpadded_size + 12;
    let u64_data = bump.alloc_slice_fill_copy(padded_size / 8 + 1, 0u64);
    &mut transmute_to_bytes_mut(u64_data)[3..padded_size + 3]
}

pub fn new_sol_account(lamports: u64, bump: &Bump) -> AccountInfo {
    new_sol_account_with_pubkey(random_pubkey(bump), lamports, bump)
}

pub fn new_sol_account_with_pubkey<'bump>(
    pubkey: &'bump Pubkey,
    lamports: u64,
    bump: &'bump Bump,
) -> AccountInfo<'bump> {
    AccountInfo::new(
        pubkey,
        true,
        false,
        bump.alloc(lamports),
        &mut [],
        &system_program::ID,
        false,
        Epoch::default(),
    )
}

pub fn new_dex_owned_account<'bump>(
    unpadded_len: usize,
    program_id: &'bump Pubkey,
    bump: &'bump Bump,
    rent: Rent,
) -> AccountInfo<'bump> {
    let data_len = unpadded_len + 12;
    new_dex_owned_account_with_lamports(unpadded_len, rent.minimum_balance(data_len), program_id, bump)
}

pub fn new_dex_owned_account_with_lamports<'bump>(
    unpadded_len: usize,
    lamports: u64,
    program_id: &'bump Pubkey,
    bump: &'bump Bump,
) -> AccountInfo<'bump> {
    AccountInfo::new(
        random_pubkey(bump),
        false,
        true,
        bump.alloc(lamports),
        allocate_dex_owned_account(unpadded_len, bump),
        program_id,
        false,
        Epoch::default(),
    )
}

pub fn new_token_mint(bump: &Bump, rent: Rent) -> AccountInfo {
    let data = bump.alloc_slice_fill_copy(Mint::LEN, 0u8);
    let mut mint = Mint::default();
    mint.is_initialized = true;
    Mint::pack(mint, data).unwrap();
    AccountInfo::new(
        random_pubkey(bump),
        false,
        true,
        bump.alloc(rent.minimum_balance(data.len())),
        data,
        &spl_token::ID,
        false,
        Epoch::default(),
    )
}

pub fn new_token_account<'bump, 'a, 'b>(
    mint_pubkey: &'a Pubkey,
    owner_pubkey: &'b Pubkey,
    balance: u64,
    bump: &'bump Bump,
    rent: Rent
) -> AccountInfo<'bump> {
    let data = bump.alloc_slice_fill_copy(SplAccount::LEN, 0u8);
    let mut account = SplAccount::default();
    account.state = spl_token::state::AccountState::Initialized;
    account.mint = *mint_pubkey;
    account.owner = *owner_pubkey;
    account.amount = balance;
    SplAccount::pack(account, data).unwrap();
    AccountInfo::new(
        random_pubkey(bump),
        false,
        true,
        bump.alloc(rent.minimum_balance(data.len())),
        data,
        &spl_token::ID,
        false,
        Epoch::default(),
    )
}

pub fn new_spl_token_program(bump: &Bump) -> AccountInfo {
    AccountInfo::new(
        &spl_token::ID,
        false,
        false,
        bump.alloc(0),
        &mut [],
        &bpf_loader::ID,
        false,
        Epoch::default(),
    )
}

fn new_rent_sysvar_account(lamports: u64, rent: Rent, bump: &Bump) -> AccountInfo {
    let data = bump.alloc_slice_fill_copy(size_of::<Rent>(), 0u8);
    let mut account_info = AccountInfo::new(
        &sysvar::rent::ID,
        false,
        false,
        bump.alloc(lamports),
        data,
        &sysvar::ID,
        false,
        Epoch::default(),
    );
    rent.to_account_info(&mut account_info).unwrap();
    account_info
}

fn new_vault_signer_account<'bump>(
    market: &AccountInfo,
    program_id: &Pubkey,
    bump: &'bump Bump,
) -> (u64, AccountInfo<'bump>) {
    for i in 0..100 {
        if let Ok(pk) = gen_vault_signer_key(i, market.key, program_id) {
            return (
                i,
                AccountInfo::new(
                    bump.alloc(pk),
                    true,
                    false,
                    bump.alloc(0),
                    &mut [],
                    &system_program::ID,
                    false,
                    Epoch::default(),
                ),
            );
        }
    }
    unreachable!();
}

pub struct MarketAccounts<'bump> {
    pub market: AccountInfo<'bump>,
    pub req_q: AccountInfo<'bump>,
    pub event_q: AccountInfo<'bump>,
    pub bids: AccountInfo<'bump>,
    pub asks: AccountInfo<'bump>,
    pub coin_vault: AccountInfo<'bump>,
    pub pc_vault: AccountInfo<'bump>,
    pub coin_mint: AccountInfo<'bump>,
    pub pc_mint: AccountInfo<'bump>,
    pub vault_signer: AccountInfo<'bump>,
    pub spl_token_program: AccountInfo<'bump>,
    pub rent_sysvar: AccountInfo<'bump>,
    pub sweep_authority: AccountInfo<'bump>,
    pub fee_receiver: AccountInfo<'bump>,
}

impl<'bump> MarketAccounts<'bump> {
    pub fn rent(&self) -> Rent {
        Rent::from_account_info(&self.rent_sysvar).unwrap()
    }
}

pub const COIN_LOT_SIZE: u64 = 100_000;
pub const PC_LOT_SIZE: u64 = 100;
pub const PC_DUST_THRESHOLD: u64 = 500;

pub fn setup_market(bump: &Bump) -> MarketAccounts {
    let rent = Rent::default();
    let rent_sysvar = new_rent_sysvar_account(100000, rent, bump);

    let program_id = random_pubkey(bump);
    let market = new_dex_owned_account(size_of::<MarketState>(), program_id, bump, rent);
    let bids = new_dex_owned_account(1 << 16, program_id, bump, rent);
    let asks = new_dex_owned_account(1 << 16, program_id, bump, rent);
    let req_q = new_dex_owned_account(640, program_id, bump, rent);
    let event_q = new_dex_owned_account(65536, program_id, bump, rent);

    let coin_mint = new_token_mint(bump, rent);
    let pc_mint = new_token_mint(bump, rent);

    let (vault_signer_nonce, vault_signer) = new_vault_signer_account(&market, program_id, bump);

    let coin_vault = new_token_account(coin_mint.key, vault_signer.key, 0, bump, rent);
    let pc_vault = new_token_account(pc_mint.key, vault_signer.key, 0, bump, rent);
    let fee_receiver = new_token_account(pc_mint.key, random_pubkey(bump), 0, bump, rent);
    let sweep_authority = new_sol_account_with_pubkey(bump.alloc(fee_sweeper::ID), 0, bump);

    let spl_token_program = new_spl_token_program(bump);

    let coin_lot_size = COIN_LOT_SIZE;
    let pc_lot_size = PC_LOT_SIZE;

    let pc_dust_threshold = PC_DUST_THRESHOLD;

    let init_instruction = initialize_market(
        market.key,
        program_id,
        coin_mint.key,
        pc_mint.key,
        coin_vault.key,
        pc_vault.key,
        bids.key,
        asks.key,
        req_q.key,
        event_q.key,
        coin_lot_size,
        pc_lot_size,
        vault_signer_nonce,
        pc_dust_threshold,
    )
    .unwrap();

    process_instruction(
        program_id,
        &[
            market.clone(),
            req_q.clone(),
            event_q.clone(),
            bids.clone(),
            asks.clone(),
            coin_vault.clone(),
            pc_vault.clone(),
            coin_mint.clone(),
            pc_mint.clone(),
            rent_sysvar.clone(),
        ],
        &init_instruction.data,
    )
    .unwrap();

    MarketAccounts {
        market,
        req_q,
        event_q,
        bids,
        asks,
        coin_vault,
        pc_vault,
        coin_mint,
        pc_mint,
        vault_signer,
        spl_token_program,
        rent_sysvar,
        fee_receiver,
        sweep_authority,
    }
}

pub fn process_instruction<'a>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'a>],
    instruction_data: &[u8],
) -> DexResult {
    let original_data: Vec<Vec<u8>> = accounts
        .iter()
        .map(|account| account.try_borrow_data().unwrap().to_vec())
        .collect();
    let result = State::process(program_id, accounts, &instruction_data);
    if result.is_err() {
        for (account, original) in accounts.iter().zip(original_data) {
            let mut data = account.try_borrow_mut_data().unwrap();
            data.copy_from_slice(&original);
        }
    }
    result
}

impl<'bump> MarketAccounts<'bump> {
    pub fn print_requests(&self) {
        println!("requests: [");
        let (header, buf) = strip_header(&self.req_q, false).unwrap();
        let requests: RequestQueue = Queue::new(header, buf);
        for request in requests.iter() {
            println!("  {:?},", request.as_view().unwrap());
        }
        println!("]");
    }

    pub fn print_events(&self) {
        println!("events: [");
        let (header, buf) = strip_header(&self.event_q, false).unwrap();
        let events: EventQueue = Queue::new(header, buf);
        for event in events.iter() {
            println!("  {:?},", event.as_view().unwrap());
        }
        println!("]");
    }
}

pub fn get_token_account_balance(account: &AccountInfo) -> u64 {
    assert_eq!(account.owner, &spl_token::ID);
    let data = account.try_borrow_mut_data().unwrap();
    let unpacked = SplAccount::unpack(&data).unwrap();
    return unpacked.amount;
}

pub struct NoSolLoggingStubs;

impl solana_program::program_stubs::SyscallStubs for NoSolLoggingStubs {
    fn sol_log(&self, _message: &str) {}
    fn sol_invoke_signed(&self,
        _instruction: &Instruction,
        _account_infos: &[AccountInfo],
        _signers_seeds: &[&[&[u8]]]) -> ProgramResult {
        unimplemented!()
    }
}
