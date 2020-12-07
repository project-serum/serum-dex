extern crate crank as serum_crank;

use serum_common::client::rpc;
use serum_common_tests::Genesis;
use serum_dex::instruction::NewOrderInstructionV1;
use serum_dex::matching::{OrderType, Side};
use serum_registry::accounts::*;
use serum_registry_client::{
    Client as RegistryClient, CreateEntityRequest, CreateEntityResponse,
    InitializeRequest as RegistrarInitializeRequest,
    InitializeResponse as RegistrarInitializeResponse, RegisterCapabilityRequest,
};
use serum_rewards_client::*;
use solana_client_gen::prelude::*;
use solana_client_gen::solana_sdk::signature::{Keypair, Signer};
use spl_token::state::Account as TokenAccount;
use std::num::NonZeroU64;

#[test]
fn lifecycle() {
    let Genesis {
        client,
        srm_mint,
        msrm_mint,
        mint_authority: _,
        god,
        god_msrm,
        god_balance_before: _,
        god_msrm_balance_before: _,
        god_owner,
    } = serum_common_tests::genesis::<Client>();

    // Initialize the Registry.
    let registry_program_id = std::env::var("TEST_REGISTRY_PROGRAM_ID")
        .unwrap()
        .parse()
        .unwrap();
    let reg_client = serum_common_tests::client_at::<RegistryClient>(registry_program_id);
    let registrar = {
        // Initialize the registrar.
        let withdrawal_timelock = 1234;
        let deactivation_timelock_premium = 1000;
        let reward_activation_threshold = 1;
        let registrar_authority = Keypair::generate(&mut OsRng);
        let RegistrarInitializeResponse { registrar, .. } = reg_client
            .initialize(RegistrarInitializeRequest {
                registrar_authority: registrar_authority.pubkey(),
                withdrawal_timelock,
                deactivation_timelock_premium,
                mint: srm_mint.pubkey(),
                mega_mint: msrm_mint.pubkey(),
                reward_activation_threshold,
            })
            .unwrap();

        // Register capabilities.
        let capability_id = 0;
        let capability_fee_bps = 1234;

        let _ = reg_client
            .register_capability(RegisterCapabilityRequest {
                registrar,
                registrar_authority: &registrar_authority,
                capability_id,
                capability_fee_bps,
            })
            .unwrap();
        registrar
    };

    // Create entity.
    let CreateEntityResponse { tx: _, entity } = reg_client
        .create_entity(CreateEntityRequest {
            node_leader: &client.payer(),
            capabilities: 1,
            stake_kind: StakeKind::Delegated,
            registrar,
        })
        .unwrap();

    let dex_program_id = std::env::var("TEST_DEX_PROGRAM_ID")
        .unwrap()
        .parse()
        .unwrap();

    // Initialize the Rewards program.
    let rewards_init_resp = client
        .initialize(InitializeRequest {
            registry_program_id,
            dex_program_id,
            registrar: registrar,
            reward_mint: srm_mint.pubkey(),
            authority: client.payer().pubkey(),
        })
        .unwrap();

    // Fund the Rewards program (so that it can payout crank fees).
    let rewards_vault_amount = 100_000_000;
    {
        let instance = client.instance(rewards_init_resp.instance).unwrap();
        rpc::transfer(
            client.rpc(),
            &god.pubkey(),
            &instance.vault,
            rewards_vault_amount,
            &god_owner,
            client.payer(),
        )
        .unwrap();
    }

    // List market SRM/MSRM.
    let market_keys = serum_crank::list_market(
        client.rpc(),
        &dex_program_id,
        client.payer(),
        &srm_mint.pubkey(),
        &msrm_mint.pubkey(),
        1_000_000,
        10_000,
    )
    .unwrap();

    // Place bid.
    {
        let mut orders = None;
        serum_crank::place_order(
            client.rpc(),
            &dex_program_id,
            client.payer(),
            &god_msrm.pubkey(),
            &market_keys,
            &mut orders,
            NewOrderInstructionV1 {
                side: Side::Bid,
                limit_price: NonZeroU64::new(500).unwrap(),
                max_qty: NonZeroU64::new(1_000).unwrap(),
                order_type: OrderType::Limit,
                client_id: 019269,
            },
        )
        .unwrap();
    }

    // Place offer.
    {
        let mut orders = None;
        serum_crank::place_order(
            client.rpc(),
            &dex_program_id,
            client.payer(),
            &god.pubkey(),
            &market_keys,
            &mut orders,
            NewOrderInstructionV1 {
                side: Side::Ask,
                limit_price: NonZeroU64::new(499).unwrap(),
                max_qty: NonZeroU64::new(1_000).unwrap(),
                order_type: OrderType::Limit,
                client_id: 985982,
            },
        )
        .unwrap();
    }

    // Match orders.
    {
        std::thread::sleep(std::time::Duration::new(15, 0));
        serum_crank::match_orders(
            client.rpc(),
            &dex_program_id,
            client.payer(),
            &market_keys,
            &god.pubkey(),
            &god_msrm.pubkey(),
        )
        .unwrap();
    }

    // Crank consume events for reward.
    let token_account_pubkey = {
        std::thread::sleep(std::time::Duration::new(15, 0));
        let consume_events_instr = serum_crank::consume_events_instruction(
            client.rpc(),
            &dex_program_id,
            &market_keys,
            &god.pubkey(),
            &god_msrm.pubkey(),
        )
        .unwrap()
        .unwrap();
        // Account for receiving the crank reward.
        let token_account_kp = rpc::create_token_account(
            client.rpc(),
            &srm_mint.pubkey(),
            &client.payer().pubkey(),
            client.payer(),
        )
        .unwrap();
        client
            .crank_relay(CrankRelayRequest {
                instance: rewards_init_resp.instance,
                token_account: token_account_kp.pubkey(),
                entity,
                entity_leader: client.payer(),
                dex_event_q: *market_keys.event_q,
                consume_events_instr,
            })
            .unwrap();
        token_account_kp.pubkey()
    };

    // Check fee was received.
    let expected_fee = 6170; // 5 events * 1234 fee.
    {
        let token_account: TokenAccount =
            rpc::get_token_account(client.rpc(), &token_account_pubkey).unwrap();
        assert_eq!(token_account.amount, expected_fee);
    }

    // Set new authority.
    let new_authority = {
        let new_authority = Keypair::generate(&mut OsRng);
        client
            .set_authority(SetAuthorityRequest {
                instance: rewards_init_resp.instance,
                new_authority: new_authority.pubkey(),
                authority: client.payer(),
            })
            .unwrap();
        let i = client.instance(rewards_init_resp.instance).unwrap();
        assert_eq!(i.authority, new_authority.pubkey());
        new_authority
    };

    // Migrate funds out.
    {
        let receiver = rpc::create_token_account(
            client.rpc(),
            &srm_mint.pubkey(),
            &client.payer().pubkey(),
            client.payer(),
        )
        .unwrap();
        client
            .migrate(MigrateRequest {
                authority: &new_authority,
                instance: rewards_init_resp.instance,
                receiver: receiver.pubkey(),
            })
            .unwrap();
        // Check receiver.
        let receiver_account =
            rpc::get_token_account::<TokenAccount>(client.rpc(), &receiver.pubkey()).unwrap();
        let expected = rewards_vault_amount - expected_fee;
        assert_eq!(receiver_account.amount, expected);
        // Check vault.
        let vault = client.vault(rewards_init_resp.instance).unwrap();
        assert_eq!(vault.amount, 0);
    }
}
