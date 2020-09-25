use rand::rngs::OsRng;
use serum_common::pack::Pack;
use serum_safe::accounts::Vesting;
use solana_client_gen::solana_sdk;
use solana_client_gen::solana_sdk::commitment_config::CommitmentConfig;
use solana_client_gen::solana_sdk::instruction::AccountMeta;
use solana_client_gen::solana_sdk::signature::{Keypair, Signer};

mod common;

#[test]
fn deposit() {
    // Given.
    //
    // An initialized safe.
    let common::lifecycle::Initialized {
        client,
        safe_acc,
        safe_srm_vault,
        safe_srm_vault_authority,
        depositor,
        depositor_balance_before,
        ..
    } = common::lifecycle::initialize();

    // When.
    //
    // A depositor performs the vesting account deposit.
    let (
        vesting_acc_kp,
        expected_beneficiary,
        expected_deposit,
        expected_end_slot,
        expected_period_count,
    ) = {
        let deposit_accs = [
            AccountMeta::new(depositor.pubkey(), false),
            AccountMeta::new(client.payer().pubkey(), true), // Owner of the depositor SPL account.
            AccountMeta::new(safe_srm_vault.pubkey(), false),
            AccountMeta::new(safe_acc.pubkey(), false),
            AccountMeta::new_readonly(spl_token::ID, false),
            AccountMeta::new_readonly(solana_sdk::sysvar::rent::ID, false),
            AccountMeta::new_readonly(solana_sdk::sysvar::clock::ID, false),
        ];
        let vesting_acc_beneficiary = Keypair::generate(&mut OsRng);
        let end_slot = 100_000;
        let period_count = 1000;
        let deposit_amount = 100;

        let (_signature, keypair) = client
            .create_account_and_deposit(
                &deposit_accs,
                vesting_acc_beneficiary.pubkey(),
                end_slot,
                period_count,
                deposit_amount,
            )
            .unwrap();
        (
            keypair,
            vesting_acc_beneficiary,
            deposit_amount,
            end_slot,
            period_count,
        )
    };

    // Then.
    //
    // The vesting account is setup properly.
    {
        let vesting_acc = {
            let account = client
                .rpc()
                .get_account_with_commitment(&vesting_acc_kp.pubkey(), CommitmentConfig::recent())
                .unwrap()
                .value
                .unwrap();
            Vesting::unpack(&account.data).unwrap()
        };
        assert_eq!(vesting_acc.safe, safe_acc.pubkey());
        assert_eq!(vesting_acc.beneficiary, expected_beneficiary.pubkey());
        assert_eq!(vesting_acc.initialized, true);
        assert_eq!(vesting_acc.end_slot, expected_end_slot);
        assert_eq!(vesting_acc.period_count, expected_period_count);
    }
    // Then.
    //
    // The depositor's SPL token account has funds reduced.
    {
        let depositor_spl_acc: spl_token::state::Account =
            serum_common::client::rpc::account_token_unpacked(client.rpc(), &depositor.pubkey());
        let expected_balance = depositor_balance_before - expected_deposit;
        assert_eq!(depositor_spl_acc.amount, expected_balance);
    }
    // Then.
    //
    // The program-owned SPL token vault has funds increased.
    {
        let safe_vault_spl_acc: spl_token::state::Account =
            serum_common::client::rpc::account_token_unpacked(
                client.rpc(),
                &safe_srm_vault.pubkey(),
            );
        assert_eq!(safe_vault_spl_acc.amount, expected_deposit);
        // Sanity check the owner of the vault account.
        assert_eq!(safe_vault_spl_acc.owner, safe_srm_vault_authority);
    }
}
