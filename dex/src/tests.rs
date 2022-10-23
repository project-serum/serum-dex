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
use state::{Market, MarketState, OpenOrders, State, ToAlignedBytes};

use crate::critbit::SlabView;
use crate::error::DexErrorCode;
use crate::instruction::SendTakeInstruction;
use crate::state::account_parser::TokenAccount;

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
    vault_signer: AccountInfo<'bump>,
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

    let mut i: u64 = 0;
    let (market_key, vault_signer_nonce, vault_signer) = loop {
        assert!(i < 100);
        let market = Pubkey::new(transmute_to_bytes(&rand::random::<[u64; 4]>()));
        new_dex_owned_account(rng, size_of::<MarketState>(), program_id, bump);
        let seeds = [market.as_ref(), bytemuck::bytes_of(&i)];
        let vault_signer_pk = match Pubkey::create_program_address(&seeds, program_id) {
            Ok(pk) => pk,
            Err(_) => {
                i += 1;
                continue;
            }
        };
        let vault_signer = AccountInfo::new(
            bump.alloc(vault_signer_pk),
            true,
            false,
            bump.alloc(1000000),
            &mut [],
            &system_program::ID,
            false,
            Epoch::default(),
        );
        break (market, i, vault_signer);
    };

    let market = AccountInfo::new(
        bump.alloc(market_key),
        false,
        true,
        bump.alloc(60_000_000_000),
        allocate_dex_owned_account(size_of::<MarketState>(), bump),
        program_id,
        false,
        Epoch::default(),
    );

    let bids = new_dex_owned_account(rng, 1 << 23, program_id, bump);
    let asks = new_dex_owned_account(rng, 1 << 23, program_id, bump);
    let req_q = new_dex_owned_account(rng, 640, program_id, bump);
    let event_q = new_dex_owned_account(rng, 65536, program_id, bump);

    let coin_mint = new_token_mint(rng, bump);
    let pc_mint = new_token_mint(rng, bump);

    let rent_sysvar = new_rent_sysvar_account(100000, Rent::default(), bump);
    let coin_vault = new_token_account(rng, &coin_mint.key, vault_signer.key, 0, bump);
    let pc_vault = new_token_account(rng, &pc_mint.key, vault_signer.key, 0, bump);

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
        None,
        None,
        None,
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
        vault_signer,
    }
}

fn layer_orders(
    dex_program_id: &Pubkey,
    start_price: u64,
    end_price: u64,
    price_step: u64,
    start_size: u64,
    size_step: u64,
    side: Side,
    instruction_accounts: &[AccountInfo],
) {
    assert!(price_step > 0 && size_step > 0);
    let mut prices = vec![];
    let mut sizes = vec![];
    match side {
        Side::Bid => {
            assert!(start_price >= end_price);
            let mut price = start_price;
            let mut size = start_size;
            while price >= end_price && price > 0 {
                prices.push(price);
                sizes.push(size);
                price -= price_step;
                size += size_step;
            }
        }
        Side::Ask => {
            assert!(start_price <= end_price);
            let mut price = start_price;
            let mut size = start_size;
            while price <= end_price {
                prices.push(price);
                sizes.push(size);
                price += price_step;
                size += size_step;
            }
        }
    }
    for (i, (p, s)) in prices.iter().zip(sizes.iter()).enumerate() {
        let new_order_instruction = MarketInstruction::NewOrderV3(NewOrderInstructionV3 {
            side,
            limit_price: NonZeroU64::new(*p).unwrap(),
            max_coin_qty: NonZeroU64::new(*s).unwrap(),
            max_native_pc_qty_including_fees: NonZeroU64::new(*s * *p).unwrap(),
            client_order_id: i as u64,
            order_type: OrderType::Limit,
            self_trade_behavior: SelfTradeBehavior::AbortTransaction,
            limit: 1,
            max_ts: i64::MAX,
        });
        let starting_balance = TokenAccount::new(&instruction_accounts[6])
            .unwrap()
            .balance()
            .unwrap();
        State::process(
            dex_program_id,
            instruction_accounts,
            &new_order_instruction.pack().clone(),
        )
        .unwrap();
        let owner = instruction_accounts[7].key;
        let ending_balance = TokenAccount::new(&instruction_accounts[6])
            .unwrap()
            .balance()
            .unwrap();
        let side_str = match side {
            Side::Bid => "BUY",
            Side::Ask => "SELL",
        };
        println!(
            "{} placed {} LIMIT {} @ {}, balance {} -> {}",
            owner, s, side_str, p, starting_balance, ending_balance
        );
    }
}

struct BBO {
    bid: u64,
    ask: u64,
    nbid: u64,
    nask: u64,
    buyer: [u64; 4],
    seller: [u64; 4],
}

fn get_bbo(
    program_id: &Pubkey,
    market: &AccountInfo,
    bids_a: &AccountInfo,
    asks_a: &AccountInfo,
) -> BBO {
    let mkt = MarketState::load(market, program_id, false).unwrap();
    let bids = mkt.load_bids_mut(bids_a).unwrap();
    let asks = mkt.load_asks_mut(asks_a).unwrap();
    let (ask, nask, seller) = match asks.find_min() {
        None => (u64::MAX, 0, [0; 4]),
        Some(h) => {
            let bo = asks.get(h).unwrap().as_leaf().unwrap();
            (bo.price().into(), bo.quantity(), bo.owner())
        }
    };
    let (bid, nbid, buyer) = match bids.find_max() {
        None => (0, 0, [0; 4]),
        Some(h) => {
            let bb = bids.get(h).unwrap().as_leaf().unwrap();
            (bb.price().into(), bb.quantity(), bb.owner())
        }
    };
    BBO {
        bid,
        ask,
        nbid,
        nask,
        buyer,
        seller,
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
        max_ts: i64::MAX,
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
        max_ts: i64::MAX,
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
        let market = Market::load(&accounts.market, &dex_program_id, false).unwrap();
        assert_eq!(identity(market.pc_fees_accrued), 0);
        assert_eq!(identity(market.pc_deposits_total), 520_000);
    }

    State::process(dex_program_id, instruction_accounts, &instruction_data).unwrap();

    {
        let market = Market::load(&accounts.market, &dex_program_id, false).unwrap();
        assert_eq!(identity(market.referrer_rebates_accrued), 32);
        assert_eq!(identity(market.pc_fees_accrued), 128);
        assert_eq!(
            market.pc_fees_accrued + market.pc_deposits_total + market.referrer_rebates_accrued,
            520_000
        );
    }
    {
        let open_orders_buyer = Market::load(&accounts.market, &dex_program_id, false)
            .unwrap()
            .load_orders_mut(&orders_account_buyer, None, &dex_program_id, None, None)
            .unwrap();
        assert_eq!(identity(open_orders_buyer.native_coin_free), 0);
        assert_eq!(identity(open_orders_buyer.native_coin_total), 0);
        assert_eq!(identity(open_orders_buyer.native_pc_free), 20_000);
        assert_eq!(identity(open_orders_buyer.native_pc_total), 520_000);
        let open_orders_seller = Market::load(&accounts.market, &dex_program_id, false)
            .unwrap()
            .load_orders_mut(&orders_account_seller, None, &dex_program_id, None, None)
            .unwrap();
        assert_eq!(identity(open_orders_seller.native_coin_free), 0);
        assert_eq!(identity(open_orders_seller.native_coin_total), 0);
        assert_eq!(identity(open_orders_seller.native_pc_free), 399840);
        assert_eq!(identity(open_orders_seller.native_pc_total), 399840);
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
        let market = Market::load(&accounts.market, &dex_program_id, false).unwrap();
        assert_eq!(identity(market.referrer_rebates_accrued), 32);
        assert_eq!(identity(market.pc_fees_accrued), 128);
        assert_eq!(
            market.pc_deposits_total + market.pc_fees_accrued + market.referrer_rebates_accrued,
            520_000
        );
    }
    {
        let open_orders_buyer = Market::load(&accounts.market, &dex_program_id, false)
            .unwrap()
            .load_orders_mut(&orders_account_buyer, None, &dex_program_id, None, None)
            .unwrap();
        assert_eq!(identity(open_orders_buyer.native_coin_free), 4_000);
        assert_eq!(identity(open_orders_buyer.native_coin_total), 4_000);
        assert_eq!(identity(open_orders_buyer.native_pc_free), 20_000);
        assert_eq!(identity(open_orders_buyer.native_pc_total), 120_000);
        let open_orders_seller = Market::load(&accounts.market, &dex_program_id, false)
            .unwrap()
            .load_orders_mut(&orders_account_seller, None, &dex_program_id, None, None)
            .unwrap();
        assert_eq!(identity(open_orders_seller.native_coin_free), 0);
        assert_eq!(identity(open_orders_seller.native_coin_total), 0);
        assert_eq!(identity(open_orders_seller.native_pc_free), 399_840);
        assert_eq!(identity(open_orders_seller.native_pc_total), 399_840);
    }
}

#[test]
fn test_ioc_new_order() {
    let mut rng = StdRng::seed_from_u64(2);
    let bump = Bump::new();

    let accounts = setup_market(&mut rng, &bump);

    let dex_program_id = accounts.market.owner;

    let owner = new_sol_account(&mut rng, 1_000_000_000, &bump);
    let orders_account =
        new_dex_owned_account(&mut rng, size_of::<OpenOrders>(), dex_program_id, &bump);
    // Account with 25 coin orders (coin lot size = 1000)
    let coin_account =
        new_token_account(&mut rng, accounts.coin_mint.key, owner.key, 25_000, &bump);
    let pc_account = new_token_account(&mut rng, accounts.pc_mint.key, owner.key, 1_000_000, &bump);
    let spl_token_program = new_spl_token_program(&bump);

    let mut instruction_accounts = bump_vec![in &bump;
        accounts.market.clone(),
        orders_account.clone(),
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
    ];
    layer_orders(
        dex_program_id,
        10_000,
        9_000,
        200,
        1,
        2,
        Side::Bid,
        instruction_accounts.as_slice(),
    );
    instruction_accounts[6] = coin_account.clone();
    layer_orders(
        dex_program_id,
        10_100,
        11_100,
        200,
        1,
        1,
        Side::Ask,
        instruction_accounts.as_slice(),
    );

    let taker = new_sol_account(&mut rng, 1_000_000_000, &bump);
    let orders_account_taker =
        new_dex_owned_account(&mut rng, size_of::<OpenOrders>(), dex_program_id, &bump);
    let taker_coin_account =
        new_token_account(&mut rng, accounts.coin_mint.key, taker.key, 6_000, &bump);
    let taker_pc_account =
        new_token_account(&mut rng, accounts.pc_mint.key, owner.key, 100_000, &bump);
    // IOC take out the 10_000 level
    let instruction_data = MarketInstruction::NewOrderV3(NewOrderInstructionV3 {
        side: Side::Ask,
        limit_price: NonZeroU64::new(10_000).unwrap(),
        max_coin_qty: NonZeroU64::new(1).unwrap(),
        max_native_pc_qty_including_fees: NonZeroU64::new(1).unwrap(),
        order_type: OrderType::ImmediateOrCancel,
        client_order_id: 0xface,
        self_trade_behavior: SelfTradeBehavior::AbortTransaction,
        limit: 1,
        max_ts: i64::MAX,
    })
    .pack();
    let instruction_accounts: &[AccountInfo] = bump_vec![in &bump;
        accounts.market.clone(),
        orders_account_taker.clone(),
        accounts.req_q.clone(),
        accounts.event_q.clone(),
        accounts.bids.clone(),
        accounts.asks.clone(),
        taker_coin_account.clone(),
        taker.clone(),
        accounts.coin_vault.clone(),
        accounts.pc_vault.clone(),
        spl_token_program.clone(),
        accounts.rent_sysvar.clone(),
    ]
    .into_bump_slice();

    State::process(dex_program_id, instruction_accounts, &instruction_data).unwrap();

    let ta = TokenAccount::new(&taker_coin_account).unwrap();
    assert_eq!(ta.balance().unwrap(), 5_000);
    let ta = TokenAccount::new(&taker_pc_account).unwrap();
    assert_eq!(ta.balance().unwrap(), 100_000);

    let BBO {
        bid,
        ask,
        nbid,
        nask,
        buyer,
        seller,
    } = get_bbo(
        dex_program_id,
        &accounts.market,
        &accounts.bids,
        &accounts.asks,
    );
    assert_eq!(bid, 9800);
    assert_eq!(ask, 10100);
    assert_eq!(nbid, 3);
    assert_eq!(nask, 1);
    assert_eq!(buyer, seller);
    // IOC take out the 10_000 level
    let instruction_data = MarketInstruction::NewOrderV3(NewOrderInstructionV3 {
        side: Side::Ask,
        limit_price: NonZeroU64::new(9_800).unwrap(),
        max_coin_qty: NonZeroU64::new(5).unwrap(),
        max_native_pc_qty_including_fees: NonZeroU64::new(1).unwrap(),
        order_type: OrderType::ImmediateOrCancel,
        client_order_id: 0xabcd,
        self_trade_behavior: SelfTradeBehavior::AbortTransaction,
        limit: 1,
        max_ts: i64::MAX,
    })
    .pack();
    let instruction_accounts: &[AccountInfo] = bump_vec![in &bump;
        accounts.market.clone(),
        orders_account_taker.clone(),
        accounts.req_q.clone(),
        accounts.event_q.clone(),
        accounts.bids.clone(),
        accounts.asks.clone(),
        taker_coin_account.clone(),
        taker.clone(),
        accounts.coin_vault.clone(),
        accounts.pc_vault.clone(),
        spl_token_program.clone(),
        accounts.rent_sysvar.clone(),
    ]
    .into_bump_slice();

    State::process(dex_program_id, instruction_accounts, &instruction_data).unwrap();

    let BBO {
        bid,
        ask,
        nbid: _,
        nask: _,
        buyer,
        seller,
    } = get_bbo(
        dex_program_id,
        &accounts.market,
        &accounts.bids,
        &accounts.asks,
    );
    assert_eq!(bid, 9600);
    if ask == 9800 {
        println!("UNDEFINED BEHAVIOR: Taker placed a limit order, but specific IOC");
    }
    // This check will fail until the bug for processing IOC orders is fixed
    assert_eq!(ask, 10100);
    assert_eq!(buyer, seller);
}

#[test]
fn test_send_take() {
    let mut rng = StdRng::seed_from_u64(3);
    let bump = Bump::new();

    let accounts = setup_market(&mut rng, &bump);

    let dex_program_id = accounts.market.owner;

    let owner = new_sol_account(&mut rng, 1_000_000_000, &bump);
    let orders_account =
        new_dex_owned_account(&mut rng, size_of::<OpenOrders>(), dex_program_id, &bump);
    // Account with 25 coin orders (coin lot size = 1000)
    let coin_account =
        new_token_account(&mut rng, accounts.coin_mint.key, owner.key, 25_000, &bump);
    let pc_account = new_token_account(&mut rng, accounts.pc_mint.key, owner.key, 1_000_000, &bump);
    let spl_token_program = new_spl_token_program(&bump);

    let mut instruction_accounts = bump_vec![in &bump;
        accounts.market.clone(),
        orders_account.clone(),
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
    ];
    let market = Market::load(&accounts.market, &dex_program_id, false).unwrap();
    let pc = market.pc_deposits_total;
    let pcf = market.pc_fees_accrued;
    let cdt = market.coin_deposits_total;
    let cf = market.coin_fees_accrued;
    println!(
        "pc_deposits_total: {}, pc_fees_accrued: {}, coin_deposits_total: {}, coin_fees_accrued: {}",
        pc, pcf, cdt, cf
    );
    drop(market);
    layer_orders(
        dex_program_id,
        10_000,
        9_000,
        200,
        1,
        2,
        Side::Bid,
        instruction_accounts.as_slice(),
    );
    let market = Market::load(&accounts.market, &dex_program_id, false).unwrap();
    let pc = market.pc_deposits_total;
    let pcf = market.pc_fees_accrued;
    let cdt = market.coin_deposits_total;
    let cf = market.coin_fees_accrued;
    println!(
        "pc_deposits_total: {}, pc_fees_accrued: {}, coin_deposits_total: {}, coin_fees_accrued: {}",
        pc, pcf, cdt, cf
    );
    drop(market);
    instruction_accounts[6] = coin_account.clone();
    layer_orders(
        dex_program_id,
        10_100,
        11_100,
        200,
        1,
        1,
        Side::Ask,
        instruction_accounts.as_slice(),
    );
    let market = Market::load(&accounts.market, &dex_program_id, false).unwrap();
    let pc = market.pc_deposits_total;
    let pcf = market.pc_fees_accrued;
    let cdt = market.coin_deposits_total;
    let cf = market.coin_fees_accrued;
    println!(
        "pc_deposits_total: {}, pc_fees_accrued: {}, coin_deposits_total: {}, coin_fees_accrued: {}",
        pc, pcf, cdt, cf
    );
    drop(market);
    let mut total_pc_on_book = 0;
    let mut p = 10_000;
    let mut s = 1;
    while p >= 9_000 {
        total_pc_on_book += p * s;
        s += 2;
        p -= 200
    }

    let taker = new_sol_account(&mut rng, 1_000_000_000, &bump);
    let taker_coin_account =
        new_token_account(&mut rng, accounts.coin_mint.key, taker.key, 6_000, &bump);
    let taker_pc_account =
        new_token_account(&mut rng, accounts.pc_mint.key, taker.key, 100_000, &bump);

    let instruction_accounts = bump_vec![in &bump;
        accounts.market.clone(),
        accounts.req_q.clone(),
        accounts.event_q.clone(),
        accounts.bids.clone(),
        accounts.asks.clone(),
        taker_coin_account.clone(),
        taker_pc_account.clone(),
        taker.clone(),
        accounts.coin_vault.clone(),
        accounts.pc_vault.clone(),
        spl_token_program.clone(),
        accounts.vault_signer.clone(),
    ];

    let starting_balance = TokenAccount::new(&taker_coin_account)
        .unwrap()
        .balance()
        .unwrap();
    let starting_pc = TokenAccount::new(&taker_pc_account)
        .unwrap()
        .balance()
        .unwrap();
    let max_coin_qty = 3;
    let limit_price = 10_299;
    let max_pc_qty = max_coin_qty * limit_price;
    let send_take_ix = MarketInstruction::SendTake(SendTakeInstruction {
        side: Side::Bid,
        limit_price: NonZeroU64::new(limit_price).unwrap(),
        max_coin_qty: NonZeroU64::new(max_coin_qty).unwrap(),
        max_native_pc_qty_including_fees: NonZeroU64::new(max_pc_qty).unwrap(),
        min_coin_qty: 0,
        min_native_pc_qty: 0,
        limit: 50,
    });
    State::process(dex_program_id, &instruction_accounts, &send_take_ix.pack()).unwrap();
    let ending_balance = TokenAccount::new(&taker_coin_account)
        .unwrap()
        .balance()
        .unwrap();
    let ending_pc = TokenAccount::new(&taker_pc_account)
        .unwrap()
        .balance()
        .unwrap();
    println!(
        "{} sends 3 MARKET BUY @ {}, matched {}, paid {}",
        taker.key,
        limit_price,
        ending_balance - starting_balance,
        starting_pc - ending_pc
    );
    let market = Market::load(&accounts.market, &dex_program_id, false).unwrap();
    let pc = market.pc_deposits_total;
    let pcf = market.pc_fees_accrued;
    let cdt = market.coin_deposits_total;
    let cf = market.coin_fees_accrued;
    println!(
        "pc_deposits_total: {}, pc_fees_accrued: {}, coin_deposits_total: {}, coin_fees_accrued: {}",
        pc, pcf, cdt, cf
    );
    drop(market);

    let tca = TokenAccount::new(&taker_coin_account).unwrap();
    assert_eq!(tca.balance().unwrap(), 7_000);
    let tpca = TokenAccount::new(&taker_pc_account).unwrap();
    // There's a default 4bps fee applied, but the fee rounds up always
    // See fees.rs:taker_fee (line 137)
    assert_eq!(tpca.balance().unwrap(), 100_000 - 10_100 - 5);
    let prev_pc_balance = tpca.balance().unwrap();

    let starting_balance = TokenAccount::new(&taker_coin_account)
        .unwrap()
        .balance()
        .unwrap();
    let starting_pc = TokenAccount::new(&taker_pc_account)
        .unwrap()
        .balance()
        .unwrap();
    let max_coin_qty = 3;
    let limit_price = 9999;
    let max_pc_qty = max_coin_qty * limit_price;
    let send_take_ix = MarketInstruction::SendTake(SendTakeInstruction {
        side: Side::Ask,
        limit_price: NonZeroU64::new(limit_price).unwrap(),
        max_coin_qty: NonZeroU64::new(max_coin_qty).unwrap(),
        max_native_pc_qty_including_fees: NonZeroU64::new(max_pc_qty).unwrap(),
        min_coin_qty: 0,
        min_native_pc_qty: 0,
        limit: 1,
    });
    State::process(dex_program_id, &instruction_accounts, &send_take_ix.pack()).unwrap();
    let ending_balance = TokenAccount::new(&taker_coin_account)
        .unwrap()
        .balance()
        .unwrap();
    let ending_pc = TokenAccount::new(&taker_pc_account)
        .unwrap()
        .balance()
        .unwrap();
    println!(
        "{} sends 3 MARKET SELL @ {}, matched {}, received {}",
        taker.key,
        limit_price,
        starting_balance - ending_balance,
        ending_pc - starting_pc
    );
    let market = Market::load(&accounts.market, &dex_program_id, false).unwrap();
    let pc = market.pc_deposits_total;
    let pcf = market.pc_fees_accrued;
    let cdt = market.coin_deposits_total;
    let cf = market.coin_fees_accrued;
    println!(
        "pc_deposits_total: {}, pc_fees_accrued: {}, coin_deposits_total: {}, coin_fees_accrued: {}",
        pc, pcf, cdt, cf
    );
    drop(market);

    let tca = TokenAccount::new(&taker_coin_account).unwrap();
    assert_eq!(tca.balance().unwrap(), 6_000);
    let tpca = TokenAccount::new(&taker_pc_account).unwrap();
    assert_eq!(
        tpca.balance().unwrap(),
        prev_pc_balance + 10_000 - 4 /* This time the fee is exactly 4 bps */
    );
    let prev_pc_balance = tpca.balance().unwrap();

    let starting_balance = TokenAccount::new(&taker_coin_account)
        .unwrap()
        .balance()
        .unwrap();
    let starting_pc = TokenAccount::new(&taker_pc_account)
        .unwrap()
        .balance()
        .unwrap();
    let max_coin_qty = 3;
    let limit_price = 9999;
    let max_pc_qty = max_coin_qty * limit_price;
    let mut send_take_ix = SendTakeInstruction {
        side: Side::Ask,
        limit_price: NonZeroU64::new(limit_price).unwrap(),
        max_coin_qty: NonZeroU64::new(max_coin_qty).unwrap(),
        max_native_pc_qty_including_fees: NonZeroU64::new(max_pc_qty).unwrap(),
        min_coin_qty: max_coin_qty,
        min_native_pc_qty: max_pc_qty,
        limit: 2,
    };
    let send_take = MarketInstruction::SendTake(send_take_ix.clone());
    assert!(State::process(dex_program_id, &instruction_accounts, &send_take.pack()).is_err());
    let ending_balance = TokenAccount::new(&taker_coin_account)
        .unwrap()
        .balance()
        .unwrap();
    let ending_pc = TokenAccount::new(&taker_pc_account)
        .unwrap()
        .balance()
        .unwrap();
    println!(
        "{} sends 3 MARKET SELL @ {}, matched {}, received {}",
        taker.key,
        limit_price,
        starting_balance - ending_balance,
        ending_pc - starting_pc
    );
    send_take_ix.limit_price = NonZeroU64::new(9800).unwrap();
    send_take_ix.min_coin_qty = 1;
    send_take_ix.min_native_pc_qty = 0;
    let send_take = MarketInstruction::SendTake(send_take_ix.clone());
    assert!(!State::process(dex_program_id, &instruction_accounts, &send_take.pack()).is_err());
    let ending_balance = TokenAccount::new(&taker_coin_account)
        .unwrap()
        .balance()
        .unwrap();
    let ending_pc = TokenAccount::new(&taker_pc_account)
        .unwrap()
        .balance()
        .unwrap();
    println!(
        "{} sends 3 MARKET SELL @ {}, matched {}, received {}",
        taker.key,
        send_take_ix.limit_price.get(),
        starting_balance - ending_balance,
        ending_pc - starting_pc
    );
    let market = Market::load(&accounts.market, &dex_program_id, false).unwrap();
    let pc = market.pc_deposits_total;
    let pcf = market.pc_fees_accrued;
    let cdt = market.coin_deposits_total;
    let cf = market.coin_fees_accrued;
    println!(
        "pc_deposits_total: {}, pc_fees_accrued: {}, coin_deposits_total: {}, coin_fees_accrued: {}",
        pc, pcf, cdt, cf
    );
    drop(market);

    let BBO {
        bid,
        ask,
        nbid,
        nask,
        buyer,
        seller,
    } = get_bbo(
        dex_program_id,
        &accounts.market,
        &accounts.bids,
        &accounts.asks,
    );
    assert_eq!(bid, 9600);
    assert_eq!(ask, 10300);
    assert_eq!(nbid, 5);
    assert_eq!(nask, 2);
    assert_eq!(buyer, seller);
    let tca = TokenAccount::new(&taker_coin_account).unwrap();
    assert_eq!(tca.balance().unwrap(), 3_000);
    let tpca = TokenAccount::new(&taker_pc_account).unwrap();
    assert_eq!(tpca.balance().unwrap(), prev_pc_balance + (3 * 9800) - 12);

    {
        let crank_accounts = bump_vec![in &bump;
            orders_account.clone(),
            accounts.market.clone(),
            accounts.event_q.clone(),
            coin_account.clone(),
            pc_account.clone(),
        ]
        .into_bump_slice_mut();
        let instruction_data = MarketInstruction::ConsumeEvents(200).pack();
        State::process(dex_program_id, crank_accounts, &instruction_data).unwrap();
    }
    {
        let open_orders = Market::load(&accounts.market, &dex_program_id, false)
            .unwrap()
            .load_orders_mut(&orders_account, None, &dex_program_id, None, None)
            .unwrap();

        // The taker sold a total of 4 coins
        assert_eq!(identity(open_orders.native_coin_free), 4_000);
        // The maker places 21 offers (1+2+...+6) and 1 was filled. Then there were 4 sells
        assert_eq!(identity(open_orders.native_coin_total), 24_000);
        assert_eq!(identity(open_orders.native_pc_free), 10100);
        assert_eq!(
            identity(open_orders.native_pc_total),
            total_pc_on_book - 10000 - (9800 * 3) + 10100
        );
    }
    let market = Market::load(&accounts.market, &dex_program_id, false).unwrap();
    let pc = market.pc_deposits_total;
    let pcf = market.pc_fees_accrued;
    let cdt = market.coin_deposits_total;
    let cf = market.coin_fees_accrued;
    println!(
        "pc_deposits_total: {}, pc_fees_accrued: {}, coin_deposits_total: {}, coin_fees_accrued: {}",
        pc, pcf, cdt, cf
    );
}

#[test]
fn test_cancel_orders() {
    let mut rng = StdRng::seed_from_u64(1);
    let bump = Bump::new();

    let accounts = setup_market(&mut rng, &bump);

    let dex_program_id = accounts.market.owner;

    let owner = new_sol_account(&mut rng, 1_000_000_000, &bump);
    let orders_account =
        new_dex_owned_account(&mut rng, size_of::<OpenOrders>(), dex_program_id, &bump);
    let pc_account = new_token_account(&mut rng, accounts.pc_mint.key, owner.key, 1_000_000, &bump);
    let spl_token_program = new_spl_token_program(&bump);

    for i in 0..3 {
        let instruction_data = MarketInstruction::NewOrderV3(NewOrderInstructionV3 {
            side: Side::Bid,
            limit_price: NonZeroU64::new(10_000).unwrap(),
            max_coin_qty: NonZeroU64::new(10).unwrap(),
            max_native_pc_qty_including_fees: NonZeroU64::new(50_000).unwrap(),
            order_type: OrderType::Limit,
            // 0x123a, 0x123b, 0x123c
            client_order_id: 0x123a + i,
            self_trade_behavior: SelfTradeBehavior::AbortTransaction,
            limit: 5,
            max_ts: i64::MAX,
        })
        .pack();

        let instruction_accounts: &[AccountInfo] = bump_vec![in &bump;
            accounts.market.clone(),
            orders_account.clone(),
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
    }

    {
        let market = Market::load(&accounts.market, &dex_program_id, false).unwrap();
        assert_eq!(identity(market.pc_fees_accrued), 0);
        assert_eq!(identity(market.pc_deposits_total), 150_000);
        let open_orders = market
            .load_orders_mut(&orders_account, None, &dex_program_id, None, None)
            .unwrap();
        assert_eq!(identity(open_orders.native_coin_free), 0);
        assert_eq!(identity(open_orders.native_coin_total), 0);
        assert_eq!(identity(open_orders.native_pc_free), 0);
        assert_eq!(identity(open_orders.native_pc_total), 150_000);
    }

    {
        // cancel 0x123a, do nothing to 0x123b, cancel 0x123c, 0x123d does not exist
        let instruction_data =
            MarketInstruction::CancelOrdersByClientIds([0x123a, 0x123d, 0, 0x123c, 0, 0, 0, 0])
                .pack();

        let instruction_accounts: &[AccountInfo] = bump_vec![in &bump;
            accounts.market.clone(),
            accounts.bids.clone(),
            accounts.asks.clone(),
            orders_account.clone(),
            owner.clone(),
            accounts.event_q.clone(),
        ]
        .into_bump_slice();

        State::process(dex_program_id, instruction_accounts, &instruction_data).unwrap();
    }

    {
        let open_orders = Market::load(&accounts.market, &dex_program_id, false)
            .unwrap()
            .load_orders_mut(&orders_account, None, &dex_program_id, None, None)
            .unwrap();
        assert_eq!(identity(open_orders.native_coin_free), 0);
        assert_eq!(identity(open_orders.native_coin_total), 0);
        assert_eq!(identity(open_orders.native_pc_free), 100_000);
        assert_eq!(identity(open_orders.native_pc_total), 150_000);
    }
}

#[test]
fn test_max_ts_order() {
    let mut rng = StdRng::seed_from_u64(1);
    let bump = Bump::new();

    let accounts = setup_market(&mut rng, &bump);

    let dex_program_id = accounts.market.owner;

    let owner = new_sol_account(&mut rng, 1_000_000_000, &bump);
    let orders_account =
        new_dex_owned_account(&mut rng, size_of::<OpenOrders>(), dex_program_id, &bump);
    let pc_account = new_token_account(&mut rng, accounts.pc_mint.key, owner.key, 1_000_000, &bump);
    let spl_token_program = new_spl_token_program(&bump);

    let instruction_data = MarketInstruction::NewOrderV3(NewOrderInstructionV3 {
        side: Side::Bid,
        limit_price: NonZeroU64::new(10_000).unwrap(),
        max_coin_qty: NonZeroU64::new(10).unwrap(),
        max_native_pc_qty_including_fees: NonZeroU64::new(50_000).unwrap(),
        order_type: OrderType::Limit,
        client_order_id: 0xabcd,
        self_trade_behavior: SelfTradeBehavior::AbortTransaction,
        limit: 5,
        max_ts: 1_649_999_999,
    })
    .pack();

    let instruction_accounts: &[AccountInfo] = bump_vec![in &bump;
        accounts.market.clone(),
        orders_account.clone(),
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

    {
        let result = State::process(dex_program_id, instruction_accounts, &instruction_data);
        let expected = Err(DexErrorCode::OrderMaxTimestampExceeded.into());
        assert_eq!(result, expected);
    }

    let instruction_data = MarketInstruction::NewOrderV3(NewOrderInstructionV3 {
        side: Side::Bid,
        limit_price: NonZeroU64::new(10_000).unwrap(),
        max_coin_qty: NonZeroU64::new(10).unwrap(),
        max_native_pc_qty_including_fees: NonZeroU64::new(50_000).unwrap(),
        order_type: OrderType::Limit,
        client_order_id: 0xabcd,
        self_trade_behavior: SelfTradeBehavior::AbortTransaction,
        limit: 5,
        max_ts: 1_650_000_000,
    })
    .pack();

    let instruction_accounts: &[AccountInfo] = bump_vec![in &bump;
        accounts.market.clone(),
        orders_account.clone(),
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

    {
        let result = State::process(dex_program_id, instruction_accounts, &instruction_data);
        let expected = Ok(());
        assert_eq!(result, expected);
    }
}

#[test]
fn test_replace_orders() {
    let mut rng = StdRng::seed_from_u64(1);
    let bump = Bump::new();

    let accounts = setup_market(&mut rng, &bump);

    let dex_program_id = accounts.market.owner;

    let owner = new_sol_account(&mut rng, 1_000_000_000, &bump);
    let orders_account =
        new_dex_owned_account(&mut rng, size_of::<OpenOrders>(), dex_program_id, &bump);
    let pc_account = new_token_account(&mut rng, accounts.pc_mint.key, owner.key, 1_000_000, &bump);
    let spl_token_program = new_spl_token_program(&bump);

    // Place orders
    // 0xabc1: 50K
    // 0xabc2: 50K
    // 0xabc3: 50K
    for client_order_id in [0xabc1, 0xabc2, 0xabc3] {
        let instruction_data = MarketInstruction::NewOrderV3(NewOrderInstructionV3 {
            side: Side::Bid,
            limit_price: NonZeroU64::new(10_000).unwrap(),
            max_coin_qty: NonZeroU64::new(10).unwrap(),
            max_native_pc_qty_including_fees: NonZeroU64::new(50_000).unwrap(),
            order_type: OrderType::Limit,
            client_order_id,
            self_trade_behavior: SelfTradeBehavior::AbortTransaction,
            limit: 5,
            max_ts: i64::MAX,
        })
        .pack();

        let instruction_accounts: &[AccountInfo] = bump_vec![in &bump;
            accounts.market.clone(),
            orders_account.clone(),
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
    }

    // Verify orders have been placed
    // Total: 150K
    {
        let market = Market::load(&accounts.market, &dex_program_id, false).unwrap();
        assert_eq!(identity(market.pc_fees_accrued), 0);
        assert_eq!(identity(market.pc_deposits_total), 150_000);
        let open_orders = market
            .load_orders_mut(&orders_account, None, &dex_program_id, None, None)
            .unwrap();
        assert_eq!(identity(open_orders.native_coin_free), 0);
        assert_eq!(identity(open_orders.native_coin_total), 0);
        assert_eq!(identity(open_orders.native_pc_free), 0);
        assert_eq!(identity(open_orders.native_pc_total), 150_000);
    }

    // Replace order 0xabc1
    // 0xabc1: 50K -> 60K (replaced)
    // 0xabc2: 50K (unchanged)
    // 0xabc3: 50K (unchanged)
    {
        let instruction_data = MarketInstruction::ReplaceOrderByClientId(NewOrderInstructionV3 {
            side: Side::Bid,
            limit_price: NonZeroU64::new(10_000).unwrap(),
            max_coin_qty: NonZeroU64::new(10).unwrap(),
            max_native_pc_qty_including_fees: NonZeroU64::new(60_000).unwrap(),
            order_type: OrderType::Limit,
            client_order_id: 0xabc1,
            self_trade_behavior: SelfTradeBehavior::AbortTransaction,
            limit: 5,
            max_ts: i64::MAX,
        })
        .pack();

        let instruction_accounts: &[AccountInfo] = bump_vec![in &bump;
            accounts.market.clone(),
            orders_account.clone(),
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
    }

    {
        let open_orders = Market::load(&accounts.market, &dex_program_id, false)
            .unwrap()
            .load_orders_mut(&orders_account, None, &dex_program_id, None, None)
            .unwrap();
        assert_eq!(identity(open_orders.native_coin_free), 0);
        assert_eq!(identity(open_orders.native_coin_total), 0);
        assert_eq!(identity(open_orders.native_pc_free), 0);
        assert_eq!(identity(open_orders.native_pc_total), 160_000);
    }

    // Replace orders 0xabc1, 0xabc3, 0xabc4
    // 0xabc1: 60K -> 70K (replaced)
    // 0xabc2: 50K (unchanged)
    // 0xabc3: 50K -> 70K (replaced)
    // 0xabc4: 70K (new)
    {
        let params = [0xabc1, 0xabc3, 0xabc4]
            .map(|client_order_id| NewOrderInstructionV3 {
                side: Side::Bid,
                limit_price: NonZeroU64::new(10_000).unwrap(),
                max_coin_qty: NonZeroU64::new(10).unwrap(),
                max_native_pc_qty_including_fees: NonZeroU64::new(70_000).unwrap(),
                order_type: OrderType::Limit,
                client_order_id,
                self_trade_behavior: SelfTradeBehavior::AbortTransaction,
                limit: 5,
                max_ts: i64::MAX,
            })
            .to_vec();

        let instruction_data = MarketInstruction::ReplaceOrdersByClientIds(params).pack();

        let instruction_accounts: &[AccountInfo] = bump_vec![in &bump;
            accounts.market.clone(),
            orders_account.clone(),
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
    }

    {
        let open_orders = Market::load(&accounts.market, &dex_program_id, false)
            .unwrap()
            .load_orders_mut(&orders_account, None, &dex_program_id, None, None)
            .unwrap();
        assert_eq!(identity(open_orders.native_coin_free), 0);
        assert_eq!(identity(open_orders.native_coin_total), 0);
        assert_eq!(identity(open_orders.native_pc_free), 0);
        assert_eq!(identity(open_orders.native_pc_total), 260_000);
    }
}
