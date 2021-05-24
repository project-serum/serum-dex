use std::convert::identity;
use std::mem::size_of;
use std::num::NonZeroU64;

use bumpalo::{collections::Vec as BumpVec, vec as bump_vec, Bump};
use rand::prelude::*;
use safe_transmute::to_bytes::{transmute_to_bytes, transmute_to_bytes_mut};
use solana_program::account_info::AccountInfo;
use solana_program::bpf_loader;
use solana_program::clock::Epoch;
use solana_program::program_pack::Pack;
use solana_program::pubkey::Pubkey;
use solana_program::rent::Rent;
use solana_program::system_program;
use solana_program::sysvar;
use solana_program::sysvar::Sysvar;
use spl_token::state::{Account, AccountState, Mint};

use instruction::{initialize_market, MarketInstruction, NewOrderInstructionV3, SelfTradeBehavior};
use matching::{OrderType, Side};
use state::gen_vault_signer_key;
use state::{MarketState, OpenOrders, State, ToAlignedBytes};

use super::*;

fn random_pubkey<'bump, G: rand::Rng>(_rng: &mut G, bump: &'bump Bump) -> &'bump Pubkey {
    bump.alloc(Pubkey::new(transmute_to_bytes(&rand::random::<[u64; 4]>())))
}

struct MarketAccounts<'bump> {
    market: AccountInfo<'bump>,
    req_q: AccountInfo<'bump>,
    event_q: AccountInfo<'bump>,
    bids: AccountInfo<'bump>,
    asks: AccountInfo<'bump>,
    coin_vault: AccountInfo<'bump>,
    pc_vault: AccountInfo<'bump>,
    coin_mint: AccountInfo<'bump>,
    pc_mint: AccountInfo<'bump>,
    rent_sysvar: AccountInfo<'bump>,
}

fn allocate_dex_owned_account(unpadded_size: usize, bump: &Bump) -> &mut [u8] {
    let padded_size = unpadded_size + 12;
    let top: usize = 0;
    let mut bottom: usize = top.wrapping_sub(padded_size + 3);
    bottom &= !0x7;
    let aligned_len_bytes = top.wrapping_sub(bottom);

    let data_vec: BumpVec<'_, u64> = bump_vec![in bump; 0u64; aligned_len_bytes >> 3];
    let data = &mut transmute_to_bytes_mut(data_vec.into_bump_slice_mut())[3..padded_size + 3];
    data
}

fn new_rent_sysvar_account<'bump>(
    lamports: u64,
    rent: Rent,
    bump: &'bump Bump,
) -> AccountInfo<'bump> {
    let data = bump_vec![in bump; 0u8; size_of::<Rent>()].into_bump_slice_mut();
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

fn new_sol_account<'bump, Gen: Rng>(
    rng: &mut Gen,
    lamports: u64,
    bump: &'bump Bump,
) -> AccountInfo<'bump> {
    AccountInfo::new(
        random_pubkey(rng, bump),
        true,
        false,
        bump.alloc(lamports),
        &mut [],
        &system_program::ID,
        false,
        Epoch::default(),
    )
}

fn new_dex_owned_account<'bump, Gen: Rng>(
    rng: &mut Gen,
    unpadded_len: usize,
    program_id: &'bump Pubkey,
    bump: &'bump Bump,
) -> AccountInfo<'bump> {
    AccountInfo::new(
        random_pubkey(rng, bump),
        false,
        true,
        bump.alloc(60_000_000_000),
        allocate_dex_owned_account(unpadded_len, bump),
        program_id,
        false,
        Epoch::default(),
    )
}

fn new_token_mint<'bump, Gen: Rng>(rng: &mut Gen, bump: &'bump Bump) -> AccountInfo<'bump> {
    let data = bump_vec![in bump; 0u8; Mint::LEN].into_bump_slice_mut();
    let mut mint = Mint::default();
    mint.is_initialized = true;
    Mint::pack(mint, data).unwrap();
    AccountInfo::new(
        random_pubkey(rng, bump),
        false,
        true,
        bump.alloc(10_000_000),
        data,
        &spl_token::ID,
        false,
        Epoch::default(),
    )
}

fn new_token_account<'bump, Gen: Rng>(
    rng: &mut Gen,
    mint_pubkey: &'bump Pubkey,
    owner_pubkey: &'bump Pubkey,
    balance: u64,
    bump: &'bump Bump,
) -> AccountInfo<'bump> {
    let data = bump_vec![in bump; 0u8; Account::LEN].into_bump_slice_mut();
    let mut account = Account::default();
    account.state = AccountState::Initialized;
    account.mint = *mint_pubkey;
    account.owner = *owner_pubkey;
    account.amount = balance;
    Account::pack(account, data).unwrap();
    AccountInfo::new(
        random_pubkey(rng, bump),
        false,
        true,
        bump.alloc(10_000_000),
        data,
        &spl_token::ID,
        false,
        Epoch::default(),
    )
}

fn new_spl_token_program<'bump>(bump: &'bump Bump) -> AccountInfo<'bump> {
    AccountInfo::new(
        &spl_token::ID,
        true,
        false,
        bump.alloc(0),
        &mut [],
        &bpf_loader::ID,
        false,
        Epoch::default(),
    )
}

fn setup_market<'bump, R: Rng>(rng: &mut R, bump: &'bump Bump) -> MarketAccounts<'bump> {
    let program_id = random_pubkey(rng, bump);
    let market = new_dex_owned_account(rng, size_of::<MarketState>(), program_id, bump);
    let bids = new_dex_owned_account(rng, 1 << 23, program_id, bump);
    let asks = new_dex_owned_account(rng, 1 << 23, program_id, bump);
    let req_q = new_dex_owned_account(rng, 640, program_id, bump);
    let event_q = new_dex_owned_account(rng, 65536, program_id, bump);

    let coin_mint = new_token_mint(rng, bump);
    let pc_mint = new_token_mint(rng, bump);

    let rent_sysvar = new_rent_sysvar_account(100000, Rent::default(), bump);

    let mut i = 0;
    let (vault_signer_nonce, vault_signer_pk) = loop {
        assert!(i < 100);
        if let Ok(pk) = gen_vault_signer_key(i, &market.key, program_id) {
            break (i, bump.alloc(pk));
        }
        i += 1;
    };

    let coin_vault = new_token_account(rng, &coin_mint.key, vault_signer_pk, 0, bump);
    let pc_vault = new_token_account(rng, &pc_mint.key, vault_signer_pk, 0, bump);

    let coin_lot_size = 1_000;
    let pc_lot_size = 1;

    let pc_dust_threshold = 5;

    let init_instruction = initialize_market(
        &market.key,
        &program_id,
        &coin_mint.key,
        &pc_mint.key,
        &coin_vault.key,
        &pc_vault.key,
        &bids.key,
        &asks.key,
        &req_q.key,
        &event_q.key,
        coin_lot_size,
        pc_lot_size,
        vault_signer_nonce,
        pc_dust_threshold,
    )
    .unwrap();

    {
        let accounts: &'bump [AccountInfo<'bump>] = bump_vec![in bump;
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
        ]
        .into_bump_slice_mut();
        State::process(&program_id, accounts, &init_instruction.data).unwrap();
    }

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
        rent_sysvar,
    }
}

#[test]
fn test_initialize_market() {
    let mut rng = StdRng::seed_from_u64(0);
    let bump = Bump::new();

    setup_market(&mut rng, &bump);
}

#[test]
fn test_new_order() {
    let mut rng = StdRng::seed_from_u64(1);
    let bump = Bump::new();

    let accounts = setup_market(&mut rng, &bump);

    let dex_program_id = accounts.market.owner;

    let owner = new_sol_account(&mut rng, 1_000_000_000, &bump);
    let orders_account_buyer =
        new_dex_owned_account(&mut rng, size_of::<OpenOrders>(), dex_program_id, &bump);
    let orders_account_seller =
        new_dex_owned_account(&mut rng, size_of::<OpenOrders>(), dex_program_id, &bump);
    let coin_account =
        new_token_account(&mut rng, accounts.coin_mint.key, owner.key, 10_000, &bump);
    let pc_account = new_token_account(&mut rng, accounts.pc_mint.key, owner.key, 1_000_000, &bump);
    let spl_token_program = new_spl_token_program(&bump);

    let instruction_data = MarketInstruction::NewOrderV3(NewOrderInstructionV3 {
        side: Side::Bid,
        limit_price: NonZeroU64::new(100_000).unwrap(),
        max_coin_qty: NonZeroU64::new(5).unwrap(),
        max_native_pc_qty_including_fees: NonZeroU64::new(520_000).unwrap(),
        order_type: OrderType::Limit,
        client_order_id: 0xabcd,
        self_trade_behavior: SelfTradeBehavior::AbortTransaction,
        limit: 5,
    })
    .pack();
    let instruction_accounts: &[AccountInfo] = bump_vec![in &bump;
        accounts.market.clone(),
        orders_account_buyer.clone(),
        accounts.req_q.clone(),
        accounts.event_q.clone(),
        accounts.bids.clone(),
        accounts.asks.clone(),
        pc_account.clone(),
        owner.clone(),
        accounts.coin_vault.clone(),
        accounts.pc_vault.clone(),
        spl_token_program.clone(),
        accounts.rent_sysvar.clone(),
    ]
    .into_bump_slice();

    State::process(dex_program_id, instruction_accounts, &instruction_data).unwrap();

    let instruction_data = MarketInstruction::NewOrderV3(NewOrderInstructionV3 {
        side: Side::Ask,
        limit_price: NonZeroU64::new(99_000).unwrap(),
        max_coin_qty: NonZeroU64::new(4).unwrap(),
        max_native_pc_qty_including_fees: NonZeroU64::new(std::u64::MAX).unwrap(),
        order_type: OrderType::Limit,
        self_trade_behavior: SelfTradeBehavior::AbortTransaction,
        client_order_id: 0,
        limit: 5,
    })
    .pack();
    let instruction_accounts = bump_vec![in &bump;
        accounts.market.clone(),
        orders_account_seller.clone(),
        accounts.req_q.clone(),
        accounts.event_q.clone(),
        accounts.bids.clone(),
        accounts.asks.clone(),
        coin_account.clone(),
        owner.clone(),
        accounts.coin_vault.clone(),
        accounts.pc_vault.clone(),
        spl_token_program.clone(),
        accounts.rent_sysvar.clone(),
    ]
    .into_bump_slice();

    {
        let market = MarketState::load(&accounts.market, &dex_program_id).unwrap();
        assert_eq!(identity(market.pc_fees_accrued), 0);
        assert_eq!(identity(market.pc_deposits_total), 520_000);
    }

    State::process(dex_program_id, instruction_accounts, &instruction_data).unwrap();

    {
        let market = MarketState::load(&accounts.market, &dex_program_id).unwrap();
        assert_eq!(identity(market.referrer_rebates_accrued), 176);
        assert_eq!(identity(market.pc_fees_accrued), 584);
        assert_eq!(
            market.pc_fees_accrued + market.pc_deposits_total + market.referrer_rebates_accrued,
            520_000
        );
    }
    {
        let open_orders_buyer = MarketState::load(&accounts.market, &dex_program_id)
            .unwrap()
            .load_orders_mut(&orders_account_buyer, None, &dex_program_id, None)
            .unwrap();
        assert_eq!(identity(open_orders_buyer.native_coin_free), 0);
        assert_eq!(identity(open_orders_buyer.native_coin_total), 0);
        assert_eq!(identity(open_orders_buyer.native_pc_free), 20_000);
        assert_eq!(identity(open_orders_buyer.native_pc_total), 520_000);
        let open_orders_seller = MarketState::load(&accounts.market, &dex_program_id)
            .unwrap()
            .load_orders_mut(&orders_account_seller, None, &dex_program_id, None)
            .unwrap();
        assert_eq!(identity(open_orders_seller.native_coin_free), 0);
        assert_eq!(identity(open_orders_seller.native_coin_total), 0);
        assert_eq!(identity(open_orders_seller.native_pc_free), 399120);
        assert_eq!(identity(open_orders_seller.native_pc_total), 399120);
    }

    {
        let crank_accounts = bump_vec![in &bump;
            orders_account_buyer.clone(),
            orders_account_seller.clone(),
            accounts.market.clone(),
            accounts.event_q.clone(),
            coin_account.clone(),
            pc_account.clone(),
        ]
        .into_bump_slice_mut();
        crank_accounts[0..2].sort_by_key(|account_info| account_info.key.to_aligned_bytes());
        let instruction_data = MarketInstruction::ConsumeEvents(200).pack();
        State::process(dex_program_id, crank_accounts, &instruction_data).unwrap();
    }

    {
        let market = MarketState::load(&accounts.market, &dex_program_id).unwrap();
        assert_eq!(identity(market.referrer_rebates_accrued), 176);
        assert_eq!(identity(market.pc_fees_accrued), 584);
        assert_eq!(
            market.pc_deposits_total + market.pc_fees_accrued + market.referrer_rebates_accrued,
            520_000
        );
    }
    {
        let open_orders_buyer = MarketState::load(&accounts.market, &dex_program_id)
            .unwrap()
            .load_orders_mut(&orders_account_buyer, None, &dex_program_id, None)
            .unwrap();
        assert_eq!(identity(open_orders_buyer.native_coin_free), 4_000);
        assert_eq!(identity(open_orders_buyer.native_coin_total), 4_000);
        assert_eq!(identity(open_orders_buyer.native_pc_free), 20_120);
        assert_eq!(identity(open_orders_buyer.native_pc_total), 120_120);
        let open_orders_seller = MarketState::load(&accounts.market, &dex_program_id)
            .unwrap()
            .load_orders_mut(&orders_account_seller, None, &dex_program_id, None)
            .unwrap();
        assert_eq!(identity(open_orders_seller.native_coin_free), 0);
        assert_eq!(identity(open_orders_seller.native_coin_total), 0);
        assert_eq!(identity(open_orders_seller.native_pc_free), 399_120);
        assert_eq!(identity(open_orders_seller.native_pc_total), 399_120);
    }
}
