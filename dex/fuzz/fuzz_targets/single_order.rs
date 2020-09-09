#![no_main]

use std::mem::size_of;

use arbitrary::Arbitrary;
use bumpalo::Bump;
use libfuzzer_sys::fuzz_target;

use serum_dex::instruction::{MarketInstruction, NewOrderInstruction};
use serum_dex::matching::{OrderType, Side};
use serum_dex::state::{strip_header, OpenOrders};
use serum_dex_fuzz::{
    get_token_account_balance, new_dex_owned_account, new_sol_account, new_token_account,
    process_instruction, setup_market, COIN_LOT_SIZE, PC_LOT_SIZE,
};

#[derive(Debug, Arbitrary)]
struct SingleOrder {
    instruction: NewOrderInstruction,
    balance: u64,
    correct_payer_account: bool,
}

fuzz_target!(|data: SingleOrder| { fuzz_place_order(data) });

fn fuzz_place_order(data: SingleOrder) {
    let bump = Bump::new();
    let market_accounts = setup_market(&bump);

    let owner = new_sol_account(10, &bump);
    let orders_account =
        new_dex_owned_account(size_of::<OpenOrders>(), market_accounts.market.owner, &bump);
    let coin_account = new_token_account(
        market_accounts.coin_mint.key,
        owner.key,
        data.balance,
        &bump,
    );
    let pc_account = new_token_account(market_accounts.pc_mint.key, owner.key, data.balance, &bump);

    let place_order_result = process_instruction(
        market_accounts.market.owner,
        &[
            market_accounts.market.clone(),
            orders_account.clone(),
            market_accounts.req_q.clone(),
            if data.correct_payer_account == (data.instruction.side == Side::Bid) {
                pc_account.clone()
            } else {
                coin_account.clone()
            },
            owner.clone(),
            market_accounts.coin_vault.clone(),
            market_accounts.pc_vault.clone(),
            market_accounts.spl_token_program.clone(),
        ],
        &MarketInstruction::NewOrder(data.instruction.clone()).pack(),
    );
    if !data.correct_payer_account {
        assert!(place_order_result.is_err());
    } else if data.instruction.side == Side::Ask {
        if data.balance / data.instruction.max_qty.get() >= COIN_LOT_SIZE {
            assert!(place_order_result.is_ok());
        } else {
            assert!(place_order_result.is_err());
        }
    } else {
        let mut balance_needed = data
            .instruction
            .limit_price
            .get()
            .saturating_mul(data.instruction.max_qty.get())
            .saturating_mul(PC_LOT_SIZE);
        balance_needed = balance_needed.saturating_add(balance_needed / 250);
        if balance_needed > data.balance || balance_needed == u64::max_value() {
            assert!(place_order_result.is_err());
        } else if balance_needed + 10 < data.balance {
            assert!(place_order_result.is_ok());
        }
    }

    if place_order_result.is_ok() && data.instruction.order_type != OrderType::ImmediateOrCancel {
        let (orders, _) = strip_header::<OpenOrders, u8>(&orders_account, false).unwrap();
        // println!("{:?}", orders);
        if data.instruction.side == Side::Bid {
            assert_eq!(orders.native_coin_free, 0);
            assert_eq!(orders.native_coin_total, 0);
            assert_eq!(orders.native_pc_free, 0);
            assert_ne!(orders.native_pc_total, 0);
        } else {
            assert_eq!(orders.native_pc_free, 0);
            assert_eq!(orders.native_pc_total, 0);
            assert_eq!(orders.native_coin_free, 0);
            assert_ne!(orders.native_coin_total, 0);
        }
    }

    process_instruction(
        market_accounts.market.owner,
        &[
            market_accounts.market.clone(),
            market_accounts.req_q.clone(),
            market_accounts.event_q.clone(),
            market_accounts.bids.clone(),
            market_accounts.asks.clone(),
            coin_account.clone(),
            pc_account.clone(),
        ],
        &MarketInstruction::MatchOrders(5).pack(),
    )
    .unwrap();

    if place_order_result.is_ok() && data.instruction.order_type != OrderType::ImmediateOrCancel {
        let (orders, _) = strip_header::<OpenOrders, u8>(&orders_account, false).unwrap();
        if data.instruction.side == Side::Bid {
            assert_eq!(orders.native_coin_free, 0);
            assert_eq!(orders.native_coin_total, 0);
            assert_eq!(orders.native_pc_free, 0);
            assert_ne!(orders.native_pc_total, 0);
        } else {
            assert_eq!(orders.native_pc_free, 0);
            assert_eq!(orders.native_pc_total, 0);
            assert_eq!(orders.native_coin_free, 0);
            assert_ne!(orders.native_coin_total, 0);
        }
    }

    process_instruction(
        market_accounts.market.owner,
        &[
            orders_account.clone(),
            market_accounts.market.clone(),
            market_accounts.event_q.clone(),
            market_accounts.coin_vault.clone(),
            market_accounts.pc_vault.clone(),
        ],
        &MarketInstruction::ConsumeEvents(5).pack(),
    )
    .unwrap();

    if place_order_result.is_ok() && data.instruction.order_type != OrderType::ImmediateOrCancel {
        let (orders, _) = strip_header::<OpenOrders, u8>(&orders_account, false).unwrap();
        // println!("{:?}", orders);
        if data.instruction.side == Side::Bid {
            assert_eq!(orders.native_coin_free, 0);
            assert_eq!(orders.native_coin_total, 0);
            // assert_eq!(orders.native_pc_free, 0);
            assert_ne!(orders.native_pc_total, 0);
        } else {
            assert_eq!(orders.native_pc_free, 0);
            assert_eq!(orders.native_pc_total, 0);
            assert_eq!(orders.native_coin_free, 0);
            assert_ne!(orders.native_coin_total, 0);
        }
    }

    let cancel_order_result = process_instruction(
        market_accounts.market.owner,
        &[
            market_accounts.market.clone(),
            orders_account.clone(),
            market_accounts.req_q.clone(),
            owner.clone(),
        ],
        &MarketInstruction::CancelOrderByClientId(data.instruction.client_id).pack(),
    );

    if place_order_result.is_ok() && data.instruction.order_type != OrderType::ImmediateOrCancel {
        assert!(cancel_order_result.is_ok());
    } else {
        assert!(cancel_order_result.is_err())
    }

    process_instruction(
        market_accounts.market.owner,
        &[
            market_accounts.market.clone(),
            market_accounts.req_q.clone(),
            market_accounts.event_q.clone(),
            market_accounts.bids.clone(),
            market_accounts.asks.clone(),
            coin_account.clone(),
            pc_account.clone(),
        ],
        &MarketInstruction::MatchOrders(5).pack(),
    )
    .unwrap();

    process_instruction(
        market_accounts.market.owner,
        &[
            orders_account.clone(),
            market_accounts.market.clone(),
            market_accounts.event_q.clone(),
            market_accounts.coin_vault.clone(),
            market_accounts.pc_vault.clone(),
        ],
        &MarketInstruction::ConsumeEvents(5).pack(),
    )
    .unwrap();

    if cancel_order_result.is_ok() {
        let (orders, _) = strip_header::<OpenOrders, u8>(&orders_account, false).unwrap();
        // println!("{:?}", orders);
        if data.instruction.side == Side::Bid {
            assert_eq!(orders.native_coin_free, 0);
            assert_eq!(orders.native_coin_total, 0);
            assert_ne!(orders.native_pc_free, 0);
            assert_ne!(orders.native_pc_total, 0);
            assert_eq!(orders.native_pc_free, orders.native_pc_total);
        } else {
            assert_eq!(orders.native_pc_free, 0);
            assert_eq!(orders.native_pc_total, 0);
            assert_ne!(orders.native_coin_free, 0);
            assert_ne!(orders.native_coin_total, 0);
            assert_eq!(orders.native_coin_free, orders.native_coin_total);
        }
    }

    process_instruction(
        market_accounts.market.owner,
        &[
            market_accounts.market.clone(),
            orders_account.clone(),
            owner.clone(),
            market_accounts.coin_vault.clone(),
            market_accounts.pc_vault.clone(),
            coin_account.clone(),
            pc_account.clone(),
            market_accounts.vault_signer.clone(),
            market_accounts.spl_token_program.clone(),
        ],
        &MarketInstruction::SettleFunds.pack(),
    )
    .unwrap();

    {
        let (orders, _) = strip_header::<OpenOrders, u8>(&orders_account, false).unwrap();
        assert_eq!(orders.native_coin_free, 0);
        assert_eq!(orders.native_coin_total, 0);
        assert_eq!(orders.native_pc_free, 0);
        assert_eq!(orders.native_pc_total, 0);
    }

    assert_eq!(get_token_account_balance(&coin_account), data.balance);
    assert_eq!(get_token_account_balance(&pc_account), data.balance);
}
