use rand::rngs::OsRng;
use serum_common::client::rpc;
use serum_common_tests::Genesis;
use serum_lockup::accounts::WhitelistEntry;
use serum_lockup_client::{
    Client as LockupClient, CreateVestingRequest, InitializeRequest as LockupInitializeRequest,
    RegistryDepositRequest, RegistryWithdrawRequest, WhitelistAddRequest,
};
use serum_registry::accounts::pending_withdrawal::PendingPayment;
use serum_registry_client::*;
use solana_client_gen::prelude::*;
use solana_client_gen::solana_sdk::program_option::COption;
use solana_client_gen::solana_sdk::pubkey::Pubkey;
use solana_client_gen::solana_sdk::signature::{Keypair, Signer};
use spl_token::state::Account as TokenAccount;

#[test]
fn lifecycle() {
    // First test initiailze.
    let genesis = serum_common_tests::genesis::<Client>();

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
    } = genesis;

    // Initialize the registrar.
    let withdrawal_timelock = 1234;
    let deactivation_timelock = 10;
    let reward_activation_threshold = 10;
    let max_stake_per_entity = 100_000_000;
    let registrar_authority = Keypair::generate(&mut OsRng);
    let stake_pid: Pubkey = std::env::var("TEST_STAKE_PROGRAM_ID")
        .unwrap()
        .parse()
        .unwrap();
    let meta_entity_program_id: Pubkey = std::env::var("TEST_META_ENTITY_PROGRAM_ID")
        .unwrap()
        .parse()
        .unwrap();
    let InitializeResponse {
        registrar,
        nonce,
        pool_vault_signer_nonce,
        pool,
        ..
    } = client
        .initialize(InitializeRequest {
            registrar_authority: registrar_authority.pubkey(),
            withdrawal_timelock,
            deactivation_timelock,
            mint: srm_mint.pubkey(),
            mega_mint: msrm_mint.pubkey(),
            reward_activation_threshold,
            max_stake_per_entity,
            pool_program_id: stake_pid,
            pool_token_decimals: 3,
        })
        .unwrap();
    // Verify initialization.
    {
        let registrar = client.registrar(&registrar).unwrap();
        assert_eq!(registrar.initialized, true);
        assert_eq!(registrar.authority, registrar_authority.pubkey());
    }

    // Initialize the lockup program, vesting account, and whitelist the
    // registrar so that we can stake locked srm.
    let (l_client, safe, vesting, vesting_beneficiary) = {
        let l_pid: Pubkey = std::env::var("TEST_LOCKUP_PROGRAM_ID")
            .unwrap()
            .parse()
            .unwrap();
        let l_client = serum_common_tests::client_at::<LockupClient>(l_pid);
        // Initialize.
        let init_resp = l_client
            .initialize(LockupInitializeRequest {
                authority: l_client.payer().pubkey(),
            })
            .unwrap();
        // Whitelist the registrar.
        l_client
            .whitelist_add(WhitelistAddRequest {
                authority: l_client.payer(),
                safe: init_resp.safe,
                entry: WhitelistEntry::new(*client.program(), Some(registrar), nonce),
            })
            .unwrap();
        // Whitelist the two staking pools.
        l_client
            .whitelist_add(WhitelistAddRequest {
                authority: l_client.payer(),
                safe: init_resp.safe,
                entry: WhitelistEntry::new(stake_pid, Some(pool), pool_vault_signer_nonce),
            })
            .unwrap();
        // TODO: whitelist the msrm pool.
        // Create vesting.
        let current_ts = client
            .rpc()
            .get_block_time(client.rpc().get_slot().unwrap())
            .unwrap();
        let deposit_amount = 1_000;
        let c_vest_resp = l_client
            .create_vesting(CreateVestingRequest {
                depositor: god.pubkey(),
                depositor_owner: &god_owner,
                safe: init_resp.safe,
                beneficiary: client.payer().pubkey(),
                end_ts: current_ts + 60,
                period_count: 10,
                deposit_amount,
            })
            .unwrap();
        (
            l_client,
            init_resp.safe,
            c_vest_resp.vesting,
            client.payer(),
        )
    };

    // Create entity.
    let node_leader = Keypair::generate(&mut OsRng);
    let node_leader_pubkey = node_leader.pubkey();
    let entity = {
        let CreateEntityResponse { tx: _, entity } = client
            .create_entity(CreateEntityRequest {
                node_leader: &node_leader,
                registrar,
                name: "".to_string(),
                about: "".to_string(),
                image_url: "".to_string(),
                meta_entity_program_id,
            })
            .unwrap();
        let entity_acc = client.entity(&entity).unwrap();
        assert_eq!(entity_acc.leader, node_leader_pubkey);
        assert_eq!(entity_acc.initialized, true);
        assert_eq!(entity_acc.balances.spt_amount, 0);
        assert_eq!(entity_acc.balances.spt_mega_amount, 0);
        entity
    };

    // Update entity.
    {
        let new_leader = Pubkey::new_rand();
        let _ = client
            .update_entity(UpdateEntityRequest {
                entity,
                leader: &node_leader,
                new_leader: Some(new_leader),
                new_metadata: None,
                registrar,
            })
            .unwrap();

        let entity_account = client.entity(&entity).unwrap();
        assert_eq!(entity_account.leader, new_leader);
    }

    // CreateMember.
    let beneficiary = &god_owner;
    let vesting_vault_authority = l_client
        .vault_authority(safe, vesting, beneficiary.pubkey())
        .unwrap();
    let member = {
        let CreateMemberResponse { tx: _, member } = client
            .create_member(CreateMemberRequest {
                entity,
                registrar,
                beneficiary,
                delegate: vesting_vault_authority,
            })
            .unwrap();

        let member_account = client.member(&member).unwrap();
        assert_eq!(member_account.initialized, true);
        assert_eq!(member_account.entity, entity);
        assert_eq!(member_account.beneficiary, beneficiary.pubkey());
        assert_eq!(
            member_account.balances.delegate.owner,
            vesting_vault_authority,
        );
        assert_eq!(member_account.balances.spt_amount, 0);
        assert_eq!(member_account.balances.spt_mega_amount, 0);
        member
    };

    // Stake intent.
    let god_acc = rpc::get_token_account::<TokenAccount>(client.rpc(), &god.pubkey()).unwrap();
    let god_balance_before = god_acc.amount;
    let current_deposit_amount = 100;
    {
        client
            .deposit(DepositRequest {
                member,
                beneficiary,
                entity,
                depositor: god.pubkey(),
                depositor_authority: &god_owner,
                registrar,
                amount: current_deposit_amount,
                pool_program_id: stake_pid,
            })
            .unwrap();
        let vault = client.current_deposit_vault(&registrar).unwrap();
        assert_eq!(current_deposit_amount, vault.amount);
        let god_acc = rpc::get_token_account::<TokenAccount>(client.rpc(), &god.pubkey()).unwrap();
        assert_eq!(god_acc.amount, god_balance_before - current_deposit_amount);
    }

    // Stake intent withdrawal.
    {
        client
            .withdraw(WithdrawRequest {
                member,
                beneficiary,
                entity,
                depositor: god.pubkey(),
                registrar,
                amount: current_deposit_amount,
                pool_program_id: stake_pid,
            })
            .unwrap();
        let vault = client.current_deposit_vault(&registrar).unwrap();
        assert_eq!(0, vault.amount);
        let god_acc = rpc::get_token_account::<TokenAccount>(client.rpc(), &god.pubkey()).unwrap();
        assert_eq!(god_acc.amount, god_balance_before);
    }

    // Stake intent from lockup.
    let l_vault_amount = l_client.vault_for(&vesting).unwrap().amount;
    {
        l_client
            .registry_deposit(RegistryDepositRequest {
                amount: current_deposit_amount,
                is_mega: false,
                registry_pid: *client.program(),
                registrar,
                member,
                entity,
                beneficiary: vesting_beneficiary,
                stake_beneficiary: beneficiary,
                vesting,
                safe,
                pool_program_id: stake_pid,
            })
            .unwrap();
        let vault = client.current_deposit_vault(&registrar).unwrap();
        assert_eq!(current_deposit_amount, vault.amount);
        let l_vault = l_client.vault_for(&vesting).unwrap();
        assert_eq!(l_vault_amount - current_deposit_amount, l_vault.amount);
    }

    // Stake intent withdrawal back to lockup.
    {
        l_client
            .registry_withdraw(RegistryWithdrawRequest {
                amount: current_deposit_amount,
                is_mega: false,
                registry_pid: *client.program(),
                registrar,
                member,
                entity,
                beneficiary: vesting_beneficiary,
                stake_beneficiary: beneficiary,
                vesting,
                safe,
                pool_program_id: stake_pid,
            })
            .unwrap();
        let vault = client.current_deposit_vault(&registrar).unwrap();
        assert_eq!(0, vault.amount);
        let l_vault = l_client.vault_for(&vesting).unwrap();
        assert_eq!(l_vault_amount, l_vault.amount);
    }

    // Activate the node, depositing 1 MSRM.
    {
        client
            .deposit(DepositRequest {
                member,
                beneficiary,
                entity,
                depositor: god_msrm.pubkey(),
                depositor_authority: &god_owner,
                registrar,
                amount: 1,
                pool_program_id: stake_pid,
            })
            .unwrap();
    }

    // Stake 1 MSRM.
    {
        let StakeResponse { tx: _ } = client
            .stake(StakeRequest {
                registrar,
                entity,
                member,
                beneficiary,
                pool_token_amount: 1,
                pool_program_id: stake_pid,
                mega: true,
            })
            .unwrap();
        let user_pool_token_acc = client.mega_pool_token(&member).unwrap().account;
        assert_eq!(user_pool_token_acc.amount, 1);
        assert_eq!(
            user_pool_token_acc.owner,
            client.vault_authority(&registrar).unwrap()
        );
        assert_eq!(
            user_pool_token_acc.delegate,
            COption::Some(beneficiary.pubkey()),
        );
        let (srm_vault, msrm_vault) = client.stake_mega_pool_asset_vaults(&registrar).unwrap();
        assert_eq!(srm_vault.amount, 0);
        assert_eq!(msrm_vault.amount, 1);
    }

    // Deposit more SRM.
    {
        client
            .deposit(DepositRequest {
                member,
                beneficiary,
                entity,
                depositor: god.pubkey(),
                depositor_authority: &god_owner,
                registrar,
                amount: current_deposit_amount,
                pool_program_id: stake_pid,
            })
            .unwrap();
    }

    // Stake the deposited SRM.
    let StakeResponse { tx: _ } = client
        .stake(StakeRequest {
            registrar,
            entity,
            member,
            beneficiary,
            pool_token_amount: current_deposit_amount,
            pool_program_id: stake_pid,
            mega: false,
        })
        .unwrap();
    let user_pool_token_acc = client.pool_token(&member).unwrap().account;
    assert_eq!(user_pool_token_acc.amount, current_deposit_amount);
    assert_eq!(
        user_pool_token_acc.owner,
        client.vault_authority(&registrar).unwrap()
    );
    assert_eq!(
        user_pool_token_acc.delegate,
        COption::Some(beneficiary.pubkey()),
    );

    let pool_vault = client.stake_pool_asset_vault(&registrar).unwrap();
    assert_eq!(pool_vault.amount, current_deposit_amount);

    let vault = client.current_deposit_vault(&registrar).unwrap();
    assert_eq!(vault.amount, 0);

    // Stake withdrawal start.
    let pending_withdrawal = {
        let StartStakeWithdrawalResponse {
            tx: _,
            pending_withdrawal,
        } = client
            .start_stake_withdrawal(StartStakeWithdrawalRequest {
                registrar,
                entity,
                member,
                beneficiary,
                spt_amount: current_deposit_amount,
                mega: false,
                pool_program_id: stake_pid,
            })
            .unwrap();

        let member_acc = client.member(&member).unwrap();
        assert_eq!(member_acc.balances.spt_amount, 0);
        assert_eq!(member_acc.balances.current_deposit, 0);

        let vault = client.current_deposit_vault(&registrar).unwrap();
        assert_eq!(vault.amount, current_deposit_amount);

        let user_pool_token = client.pool_token(&member).unwrap().account;
        assert_eq!(user_pool_token.amount, 0);

        let pool_vault = client.stake_pool_asset_vault(&registrar).unwrap();
        assert_eq!(pool_vault.amount, 0);

        // PendingWithdrawal.
        let pending_withdrawal_acc = client.pending_withdrawal(&pending_withdrawal).unwrap();
        assert_eq!(pending_withdrawal_acc.initialized, true);
        assert_eq!(pending_withdrawal_acc.member, member);
        assert_eq!(
            pending_withdrawal_acc.end_ts,
            pending_withdrawal_acc.start_ts + deactivation_timelock
        );
        assert_eq!(pending_withdrawal_acc.spt_amount, current_deposit_amount);
        assert_eq!(pending_withdrawal_acc.pool, pool);
        assert_eq!(
            pending_withdrawal_acc.payment,
            PendingPayment {
                asset_amount: current_deposit_amount,
                mega_asset_amount: 0,
            }
        );
        pending_withdrawal
    };

    std::thread::sleep(std::time::Duration::from_millis(1000 * 15));

    // Stake Withdrawal end.
    {
        client
            .end_stake_withdrawal(EndStakeWithdrawalRequest {
                registrar,
                member,
                entity,
                beneficiary,
                pending_withdrawal,
            })
            .unwrap();
        let vault = client.current_deposit_vault(&registrar).unwrap();
        assert_eq!(vault.amount, current_deposit_amount);

        let member = client.member(&member).unwrap();
        assert_eq!(member.balances.spt_amount, 0);
        assert_eq!(member.balances.current_deposit, current_deposit_amount);
    }

    // Withdraw MSRM.
    {
        let token_account = rpc::create_token_account(
            client.rpc(),
            &msrm_mint.pubkey(),
            &client.payer().pubkey(),
            client.payer(),
        )
        .unwrap()
        .pubkey();
        client
            .deposit(DepositRequest {
                member,
                beneficiary,
                entity,
                depositor: god_msrm.pubkey(),
                depositor_authority: &god_owner,
                registrar,
                amount: 2,
                pool_program_id: stake_pid,
            })
            .unwrap();
        client
            .withdraw(WithdrawRequest {
                member,
                beneficiary,
                entity,
                depositor: token_account,
                registrar,
                amount: 1,
                pool_program_id: stake_pid,
            })
            .unwrap();
        let token = rpc::get_token_account::<TokenAccount>(client.rpc(), &token_account).unwrap();
        assert_eq!(token.amount, 1);
    }

    // Entity switch.
    {
        let node_leader = Keypair::generate(&mut OsRng);
        // Create new entity.
        let CreateEntityResponse {
            tx: _,
            entity: new_entity,
        } = client
            .create_entity(CreateEntityRequest {
                node_leader: &node_leader,
                registrar,
                name: "".to_string(),
                about: "".to_string(),
                image_url: "".to_string(),
                meta_entity_program_id,
            })
            .unwrap();
        // Switch over to it.
        client
            .switch_entity(SwitchEntityRequest {
                member,
                entity,
                new_entity,
                beneficiary,
                registrar,
                pool_program_id: stake_pid,
            })
            .unwrap();

        let member = client.member(&member).unwrap();
        assert_eq!(member.entity, new_entity);
    }
}
