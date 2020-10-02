use common::lifecycle;
use rand::rngs::OsRng;
use serum_common::pack::Pack;
use serum_safe::accounts::Vesting;
use serum_safe::client::Client;
use serum_safe::error::SafeErrorCode;
use solana_client_gen::solana_sdk::commitment_config::CommitmentConfig;
use solana_client_gen::solana_sdk::instruction::AccountMeta;
use solana_client_gen::solana_sdk::signature::Keypair;
use solana_client_gen::solana_sdk::signature::Signer;
use solana_client_gen::solana_sdk::sysvar;
use spl_token::pack::Pack as TokenPack;

mod common;

// Note: there's no way to programatically control slot time so when testing
//       we're stuck with just waiting actual wall clock time. In theory
//       this can make the tests brittle and break for unexpected reasons,
//       e.g., the transaction gets mined at a slot that's too late and so
//       the contract rejects it. Not an issue so far but it is something to be
//       aware of.

// Summary.
//
// * Vesting amount of 100 for first period.
// * First vesting period passes.
// * Withdraw 10 SRM.
//
// Should receive the SRM.
#[test]
fn withdraw() {
    withdraw_test(WithdrawTestParams {
        test_type: TestType::Normal,
        deposit_amount: 100,
        end_slot_offset: 100,
        period_count: 10,
        expected_vesting_balance: 90,
        withdraw_amount: 10,
        expected_spl_balance: 10,
        slot_wait_offset: Some(10),
        error_code: None,
    });
}

// Summary.
//
// * Vesting amount of 100.
// * First vesting period passes.
// * Withdraw 50.
//
// Should not receive anything.
#[test]
fn withdraw_more_than_vested() {
    withdraw_test(WithdrawTestParams {
        test_type: TestType::Normal,
        deposit_amount: 100,
        end_slot_offset: 100,
        period_count: 10,
        expected_vesting_balance: 100,
        withdraw_amount: 50,
        expected_spl_balance: 0,
        slot_wait_offset: Some(10),
        error_code: Some(SafeErrorCode::InsufficientWithdrawalBalance.into()),
    })
}

// Summary.
//
// * Vesting amount of 100.
// * First vesting period does not pass.
// * Withdraw 1.
//
// Should not receive anything.
#[test]
fn withdraw_without_vesting() {
    withdraw_test(WithdrawTestParams {
        test_type: TestType::Normal,
        deposit_amount: 100,
        end_slot_offset: 100_000,
        period_count: 2,
        expected_vesting_balance: 100,
        withdraw_amount: 1,
        expected_spl_balance: 0,
        slot_wait_offset: None,
        error_code: Some(SafeErrorCode::InsufficientWithdrawalBalance.into()),
    })
}

// Summary.
//
// * Vesting amount of 100.
// * Mint 2 lSRM.
// * Vesting period passes.
// * Withdraw 20 SRM.
//
// Should receive the SRM.
#[test]
fn withdraw_some_when_locked_outstanding() {
    withdraw_test(WithdrawTestParams {
        test_type: TestType::LsrmMinted(2),
        deposit_amount: 100,
        end_slot_offset: 100,
        period_count: 10,
        expected_vesting_balance: 80,
        withdraw_amount: 20,
        expected_spl_balance: 20,
        slot_wait_offset: Some(20),
        error_code: None,
    })
}

// Summary.
//
// * Vesting amount of 100.
// * Mint 2 LSRM.
// * Vesting schedule passes entirely.
// * Withdraw 99 SRM.
//
// Should not receive anything.
#[test]
fn withdraw_all_when_locked_outstanding() {
    withdraw_test(WithdrawTestParams {
        test_type: TestType::LsrmMinted(2),
        deposit_amount: 100,
        end_slot_offset: 50,
        period_count: 10,
        expected_vesting_balance: 100,
        withdraw_amount: 99,
        expected_spl_balance: 0,
        slot_wait_offset: Some(10),
        error_code: Some(SafeErrorCode::InsufficientWithdrawalBalance.into()),
    })
}

fn withdraw_test(params: WithdrawTestParams) {
    let WithdrawTestParams {
        test_type,
        deposit_amount,
        end_slot_offset,
        period_count,
        expected_vesting_balance,
        withdraw_amount,
        expected_spl_balance,
        slot_wait_offset,
        error_code,
    } = params;
    let current_slot = serum_common_tests::client::<Client>()
        .rpc()
        .get_slot()
        .unwrap();
    let end_slot = end_slot_offset + current_slot;
    // Given.
    //
    // A vesting account.
    let StartState {
        client,
        vesting_acc,
        vesting_acc_beneficiary,
        safe_acc,
        safe_srm_vault,
        safe_srm_vault_authority,
        srm_mint,
        ..
    } = start_state(test_type, deposit_amount, end_slot, period_count);
    // And.
    //
    // An empty SRM SPL token account.
    let beneficiary_srm_spl_acc = serum_common::client::rpc::create_token_account(
        client.rpc(),
        &srm_mint.pubkey(),
        &vesting_acc_beneficiary.pubkey(),
        client.payer(),
    )
    .unwrap();

    // When.
    //
    // The vesting period passes (or doesn't if set to None).
    if let Some(slot_wait_offset) = slot_wait_offset {
        let slot_wait = current_slot + slot_wait_offset;
        common::blockchain::pass_time(client.rpc(), slot_wait);
    }
    // And.
    //
    // I withdraw from the vesting account *to* the empty SPL token account.
    {
        let accounts = [
            AccountMeta::new_readonly(vesting_acc_beneficiary.pubkey(), true),
            AccountMeta::new(vesting_acc, false),
            AccountMeta::new(beneficiary_srm_spl_acc.pubkey(), false),
            AccountMeta::new(safe_srm_vault.pubkey(), false),
            AccountMeta::new_readonly(safe_srm_vault_authority, false),
            AccountMeta::new(safe_acc, false),
            AccountMeta::new_readonly(spl_token::ID, false),
            AccountMeta::new_readonly(sysvar::clock::ID, false),
        ];
        let signers = [&vesting_acc_beneficiary, client.payer()];
        let r = client.withdraw_with_signers(&signers, &accounts, withdraw_amount);
        if error_code.is_some() {
            match r {
                Ok(_) => panic!("expected error code from withdrawal"),
                Err(client_error) => {
                    assert_eq!(client_error.error_code(), error_code);
                }
            };
        }
    };
    // Then.
    //
    // I should have SRM in my account.
    {
        let spl_acc = {
            let account = client
                .rpc()
                .get_account_with_commitment(
                    &beneficiary_srm_spl_acc.pubkey(),
                    CommitmentConfig::recent(),
                )
                .unwrap()
                .value
                .unwrap();
            spl_token::state::Account::unpack_from_slice(&account.data).unwrap()
        };
        assert_eq!(spl_acc.amount, expected_spl_balance);
    }

    // Then.
    //
    // My vesting account amounts should be updated.
    {
        let vesting_acc = {
            let account = client
                .rpc()
                .get_account_with_commitment(&vesting_acc, CommitmentConfig::recent())
                .unwrap()
                .value
                .unwrap();
            Vesting::unpack(&account.data).unwrap()
        };
        assert_eq!(vesting_acc.balance, expected_vesting_balance);
    }
}

// Alisasing LsrmMinted type here because we don't use the `lsrm` field in these
// tests and so we can avoid making another needless type.
type StartState = lifecycle::LsrmMinted;

fn start_state(
    test_type: TestType,
    deposit_amount: u64,
    end_slot: u64,
    period_count: u64,
) -> StartState {
    match test_type {
        TestType::LsrmMinted(lsrm_count) => {
            lifecycle::mint_lsrm(lsrm_count, deposit_amount, end_slot, period_count)
        }
        TestType::Normal => {
            let lifecycle::Deposited {
                client,
                vesting_acc,
                vesting_acc_beneficiary,
                safe_acc,
                safe_srm_vault,
                safe_srm_vault_authority,
                srm_mint,
                deposit_amount,
                end_slot,
                period_count,
                ..
            } = lifecycle::deposit_with_schedule(deposit_amount, end_slot, period_count);
            // Dummy keypair to stuff into type. Not used.
            let lsrm_token_acc_owner = Keypair::generate(&mut OsRng);
            lifecycle::LsrmMinted {
                client,
                vesting_acc,
                vesting_acc_beneficiary,
                safe_acc,
                safe_srm_vault,
                safe_srm_vault_authority,
                srm_mint,
                lsrm: vec![],
                lsrm_token_acc_owner,
                deposit_amount,
                end_slot,
                period_count,
            }
        }
    }
}

struct WithdrawTestParams {
    test_type: TestType,
    deposit_amount: u64,
    end_slot_offset: u64,
    period_count: u64,
    expected_vesting_balance: u64,
    withdraw_amount: u64,
    expected_spl_balance: u64,
    slot_wait_offset: Option<u64>,
    error_code: Option<u32>,
}

enum TestType {
    Normal,
    LsrmMinted(usize),
}
