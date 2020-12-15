extern crate crank as serum_crank;

use anyhow::Result;
use serum_common::client::rpc;
use serum_common_tests::Genesis;
use serum_dex::instruction::NewOrderInstructionV1;
use serum_dex::matching::{OrderType, Side};
use serum_registry_client::{
    Client as RegistryClient, CreateEntityRequest, CreateEntityResponse, CreateMemberRequest,
    CreateMemberResponse, DepositRequest, InitializeRequest as RegistrarInitializeRequest,
    InitializeResponse as RegistrarInitializeResponse, StakeRequest,
};
use serum_registry_rewards_client::*;
use solana_client_gen::prelude::*;
use solana_client_gen::solana_sdk::signature::{Keypair, Signer};
use spl_token::state::Account as TokenAccount;
use std::num::NonZeroU64;

#[test]
fn lifecycle() -> Result<()> {
    let (client, genesis) = serum_common_tests::genesis::<Client>();
    let Genesis {
        srm_mint,
        msrm_mint,
        god,
        god_msrm,
        ..
    } = genesis;
    let dex_program_id = std::env::var("TEST_DEX_PROGRAM_ID")?.parse()?;

    // Initialize the Registry.
    let registry_program_id = std::env::var("TEST_REGISTRY_PROGRAM_ID")?.parse()?;
    let reg_client = serum_common_tests::client_at::<RegistryClient>(registry_program_id);

    // Initialize the registrar.
    let RegistrarInitializeResponse { registrar, .. } =
        reg_client.initialize(RegistrarInitializeRequest {
            registrar_authority: client.payer().pubkey(),
            withdrawal_timelock: 1234,
            deactivation_timelock: 1234,
            mint: srm_mint,
            mega_mint: msrm_mint,
            max_stake_per_entity: 1_000_000_000_000_000,
            stake_rate: 1,
            stake_rate_mega: 1,
        })?;

    // Create entity--and subsequently activate it so that it can receive
    // rewards via cranking.
    let CreateEntityResponse { tx: _, entity } = reg_client.create_entity(CreateEntityRequest {
        node_leader: &client.payer(),
        registrar,
        metadata: None,
    })?;
    // Create member.
    let CreateMemberResponse { member, .. } = reg_client.create_member(CreateMemberRequest {
        entity,
        registrar,
        beneficiary: client.payer(),
        delegate: Pubkey::new_from_array([0; 32]),
    })?;
    // Deposit.
    reg_client.deposit(DepositRequest {
        member,
        beneficiary: client.payer(),
        entity,
        depositor: god_msrm,
        depositor_authority: &client.payer(),
        registrar,
        amount: 1,
    })?;
    // Stake to activate it.
    reg_client.stake(StakeRequest {
        registrar,
        entity,
        member,
        beneficiary: client.payer(),
        pool_token_amount: 1,
        mega: true,
        balance_id: client.payer().pubkey(),
    })?;

    // Initialize the Rewards program.
    let rewards_init_resp = client.initialize(InitializeRequest {
        registry_program_id,
        dex_program_id,
        registrar: registrar,
        reward_mint: srm_mint,
        authority: client.payer().pubkey(),
        fee_rate: 2,
    })?;

    // Fund the Rewards program (so that it can payout crank fees).
    let rewards_vault_amount = 100_000_000;
    {
        let instance = client.instance(rewards_init_resp.instance).unwrap();
        rpc::transfer(
            client.rpc(),
            &god,
            &instance.vault,
            rewards_vault_amount,
            &client.payer(),
            client.payer(),
        )?;
    }

    // List market SRM/MSRM.
    let market_keys = serum_crank::list_market(
        client.rpc(),
        &dex_program_id,
        client.payer(),
        &srm_mint,
        &msrm_mint,
        1_000_000,
        10_000,
    )?;

    // Place bid.
    let mut orders = None;
    serum_crank::place_order(
        client.rpc(),
        &dex_program_id,
        client.payer(),
        &god_msrm,
        &market_keys,
        &mut orders,
        NewOrderInstructionV1 {
            side: Side::Bid,
            limit_price: NonZeroU64::new(500).unwrap(),
            max_qty: NonZeroU64::new(1_000).unwrap(),
            order_type: OrderType::Limit,
            client_id: 019269,
        },
    )?;

    // Place offer.
    let mut orders = None;
    serum_crank::place_order(
        client.rpc(),
        &dex_program_id,
        client.payer(),
        &god,
        &market_keys,
        &mut orders,
        NewOrderInstructionV1 {
            side: Side::Ask,
            limit_price: NonZeroU64::new(499).unwrap(),
            max_qty: NonZeroU64::new(1_000).unwrap(),
            order_type: OrderType::Limit,
            client_id: 985982,
        },
    )?;

    // Match orders.
    std::thread::sleep(std::time::Duration::new(15, 0));
    serum_crank::match_orders(
        client.rpc(),
        &dex_program_id,
        client.payer(),
        &market_keys,
        &god,
        &god_msrm,
    )?;

    // Crank consume events for reward.
    let token_account_pubkey = {
        std::thread::sleep(std::time::Duration::new(15, 0));
        let consume_events_instr = serum_crank::consume_events_instruction(
            client.rpc(),
            &dex_program_id,
            &market_keys,
            &god,
            &god_msrm,
        )?
        .unwrap();
        // Account for receiving the crank reward.
        let token_account_kp = rpc::create_token_account(
            client.rpc(),
            &srm_mint,
            &client.payer().pubkey(),
            client.payer(),
        )?;
        client.crank_relay(CrankRelayRequest {
            instance: rewards_init_resp.instance,
            token_account: token_account_kp.pubkey(),
            entity,
            dex_event_q: *market_keys.event_q,
            consume_events_instr,
        })?;
        token_account_kp.pubkey()
    };

    // Check fee was received.
    let expected_fee = 10; // 5 events * 2 fee.
    let token_account: TokenAccount = rpc::get_token_account(client.rpc(), &token_account_pubkey)?;
    assert_eq!(token_account.amount, expected_fee);

    // Set new authority.
    let new_authority = {
        let new_authority = Keypair::generate(&mut OsRng);
        client.set_authority(SetAuthorityRequest {
            instance: rewards_init_resp.instance,
            new_authority: new_authority.pubkey(),
            authority: client.payer(),
        })?;
        let i = client.instance(rewards_init_resp.instance).unwrap();
        assert_eq!(i.authority, new_authority.pubkey());
        new_authority
    };

    // Migrate funds out.
    let receiver = rpc::create_token_account(
        client.rpc(),
        &srm_mint,
        &client.payer().pubkey(),
        client.payer(),
    )?;

    client.migrate(MigrateRequest {
        authority: &new_authority,
        instance: rewards_init_resp.instance,
        receiver: receiver.pubkey(),
    })?;

    // Check receiver.
    let receiver_account =
        rpc::get_token_account::<TokenAccount>(client.rpc(), &receiver.pubkey())?;
    let expected = rewards_vault_amount - expected_fee;
    assert_eq!(receiver_account.amount, expected);

    // Check vault.
    let vault = client.vault(rewards_init_resp.instance)?;
    assert_eq!(vault.amount, 0);

    Ok(())
}
