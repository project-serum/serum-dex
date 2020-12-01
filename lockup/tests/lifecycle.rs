use rand::rngs::OsRng;
use serum_common::client::rpc;
use serum_common::pack::Pack;
use serum_lockup::accounts::Safe;
use serum_lockup_client::*;
use solana_client_gen::prelude::*;
use solana_client_gen::solana_sdk::signature::{Keypair, Signer};
use spl_token::state::Account as TokenAccount;

#[test]
fn lifecycle() {
    // Given.
    let serum_common_tests::Genesis {
        client,
        srm_mint,
        god: depositor,
        god_balance_before: depositor_balance_before,
        ..
    } = serum_common_tests::genesis::<Client>();

    // When.
    //
    // I create the safe account and initialize it.
    let safe_authority = Keypair::generate(&mut OsRng);
    let InitializeResponse {
        safe: safe_acc,
        whitelist,
        ..
    } = client
        .initialize(InitializeRequest {
            authority: safe_authority.pubkey(),
        })
        .unwrap();

    // Then.
    //
    // The Safe should be setup.
    {
        let account = client
            .rpc()
            .get_account_with_commitment(&safe_acc, CommitmentConfig::recent())
            .unwrap()
            .value
            .unwrap();
        let safe = Safe::unpack(&account.data).unwrap();
        assert_eq!(&account.owner, client.program());
        assert_eq!(account.data.len(), Safe::default().size().unwrap() as usize);
        assert_eq!(safe.authority, safe_authority.pubkey());
        assert_eq!(safe.initialized, true);
        assert_eq!(safe.whitelist, whitelist);
    };

    // CreateVesting.
    let (vesting, vesting_acc, expected_beneficiary) = {
        let vesting_acc_beneficiary = Keypair::generate(&mut OsRng);
        let current_ts = client
            .rpc()
            .get_block_time(client.rpc().get_slot().unwrap())
            .unwrap();
        let end_ts = {
            let end_ts_offset = 100;
            end_ts_offset + current_ts
        };
        let period_count = 10;
        let deposit_amount = 100;
        // When.
        //
        // A depositor performs the vesting account deposit.
        let CreateVestingResponse { tx: _, vesting } = client
            .create_vesting(CreateVestingRequest {
                depositor: depositor.pubkey(),
                depositor_owner: client.payer(),
                safe: safe_acc,
                beneficiary: vesting_acc_beneficiary.pubkey(),
                end_ts,
                period_count,
                deposit_amount,
            })
            .unwrap();

        // Then.
        //
        // The vesting account is setup properly.
        let vesting_acc = client.vesting(&vesting).unwrap();
        assert_eq!(vesting_acc.safe, safe_acc);
        assert_eq!(vesting_acc.beneficiary, vesting_acc_beneficiary.pubkey());
        assert_eq!(vesting_acc.initialized, true);
        assert_eq!(vesting_acc.end_ts, end_ts);
        assert_eq!(vesting_acc.period_count, period_count);
        assert_eq!(vesting_acc.whitelist_owned, 0);
        // Then.
        //
        // The depositor's SPL token account has funds reduced.
        let depositor_spl_acc: spl_token::state::Account =
            rpc::account_token_unpacked(client.rpc(), &depositor.pubkey());
        let expected_balance = depositor_balance_before - deposit_amount;
        assert_eq!(depositor_spl_acc.amount, expected_balance);
        // Then.
        //
        // The program-owned SPL token vault has funds increased.
        let vault = client.vault_for(&vesting).unwrap();
        assert_eq!(vault.amount, deposit_amount);
        // Sanity check the owner of the vault account.
        assert_eq!(
            vault.owner,
            client
                .vault_authority(safe_acc, vesting, vesting_acc_beneficiary.pubkey())
                .unwrap()
        );
        (vesting, vesting_acc, vesting_acc_beneficiary)
    };

    // Wait for a vesting period to lapse.
    {
        let wait_ts = vesting_acc.start_ts + 10;
        pass_time(client.rpc(), wait_ts);
    }

    // Withdraw 10 SRM.
    //
    // Current state:
    //
    // * original-deposit-amount 100
    // * balance: 97
    // * stake-amount/whitelist_owned: 3
    // * vested-amount: ~10 (depends on variance in ts time as tests run, this
    //                       is a lower bound.)
    {
        let bene_tok_acc = rpc::create_token_account(
            client.rpc(),
            &srm_mint.pubkey(),
            &expected_beneficiary.pubkey(),
            client.payer(),
        )
        .unwrap();

        let withdraw_amount = 10;

        let _ = client
            .withdraw(WithdrawRequest {
                beneficiary: &expected_beneficiary,
                vesting,
                token_account: bene_tok_acc.pubkey(),
                safe: safe_acc,
                amount: withdraw_amount,
            })
            .unwrap();

        // The SRM account should be increased.
        let bene_tok =
            rpc::account_token_unpacked::<TokenAccount>(client.rpc(), &bene_tok_acc.pubkey());
        assert_eq!(bene_tok.amount, withdraw_amount);
    }
}

pub fn pass_time(client: &RpcClient, slot_num: i64) {
    loop {
        let current_slot = client.get_block_time(client.get_slot().unwrap()).unwrap();
        if current_slot >= slot_num {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}
