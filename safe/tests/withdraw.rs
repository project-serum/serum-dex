use common::assert::assert_eq_vec;
use common::lifecycle;
use rand::rngs::OsRng;
use serum_common::pack::Pack;
use serum_safe::accounts::Vesting;
use serum_safe::error::SafeErrorCode;
use solana_client_gen::solana_sdk::commitment_config::CommitmentConfig;
use solana_client_gen::solana_sdk::instruction::AccountMeta;
use solana_client_gen::solana_sdk::signature::Keypair;
use solana_client_gen::solana_sdk::signature::Signer;
use solana_client_gen::solana_sdk::sysvar;
use spl_token::pack::Pack as TokenPack;

mod common;

// Summary.
//
// * Vesting amount of 30 for first period.
// * First vesting period passes.
// * Withdraw 30 SRM.
//
// Should receive the SRM.
#[test]
fn withdraw() {
    withdraw_test(WithdrawTestParams {
        test_type: TestType::Normal,
        vesting_slot_offsets: vec![1, 100_000, 200_000],
        vesting_amounts: vec![30, 40, 50],
        expected_vesting_amounts: vec![0, 40, 50],
        expected_withdraw_amount: 30,
        expected_spl_balance: 30,
        slot_wait_index: Some(0),
        error_code: None,
    })
}

// Summary.
//
// * Vesting amount of 30.
// * Vesting period passes.
// * Withdraw 31.
//
// Should not receive anything.
#[test]
fn withdraw_more_than_vested() {
    withdraw_test(WithdrawTestParams {
        test_type: TestType::Normal,
        vesting_slot_offsets: vec![1, 100_000, 200_000],
        vesting_amounts: vec![30, 40, 50],
        expected_vesting_amounts: vec![30, 40, 50],
        expected_withdraw_amount: 31,
        expected_spl_balance: 0,
        slot_wait_index: Some(0),
        error_code: Some(SafeErrorCode::InsufficientBalance.into()),
    })
}

// Summary.
//
// * Vesting amount of 30.
// * Vesting period does not pass.
// * Withdraw 1.
//
// Should not receive anything.
#[test]
fn withdraw_without_vesting() {
    withdraw_test(WithdrawTestParams {
        test_type: TestType::Normal,
        vesting_slot_offsets: vec![99_000, 100_000, 200_000],
        vesting_amounts: vec![30, 40, 50],
        expected_vesting_amounts: vec![30, 40, 50],
        expected_withdraw_amount: 100,
        expected_spl_balance: 0,
        slot_wait_index: None,
        error_code: Some(SafeErrorCode::InsufficientBalance.into()),
    })
}

// Summary.
//
// * Vesting amount of 30.
// * Mint 2 lSRM.
// * Vesting period passes.
// * Withdraw 20 SRM.
//
// Should receive the SRM.
#[test]
fn withdraw_some_when_locked_srm_outstanding() {
    withdraw_test(WithdrawTestParams {
        test_type: TestType::LsrmMinted(2),
        vesting_slot_offsets: vec![1, 100_000, 200_000],
        vesting_amounts: vec![30, 40, 50],
        expected_vesting_amounts: vec![10, 40, 50],
        expected_withdraw_amount: 20,
        expected_spl_balance: 20,
        slot_wait_index: Some(0),
        error_code: None,
    })
}

// Summary.
//
// * Vesting amount of 30.
// * Mint 2 LSRM.
// * Vesting period passes.
// * Withdraw 30 SRM.
//
// Should not receive anything.
#[test]
fn withdraw_all_when_locked_srm_outstanding() {
    withdraw_test(WithdrawTestParams {
        test_type: TestType::LsrmMinted(2),
        vesting_slot_offsets: vec![1, 100_000, 200_000],
        vesting_amounts: vec![30, 40, 50],
        expected_vesting_amounts: vec![30, 40, 50],
        expected_withdraw_amount: 30,
        expected_spl_balance: 0,
        slot_wait_index: Some(0),
        error_code: Some(SafeErrorCode::InsufficientBalance.into()),
    })
}

fn withdraw_test(params: WithdrawTestParams) {
    let WithdrawTestParams {
        test_type,
        vesting_slot_offsets,
        vesting_amounts,
        expected_vesting_amounts,
        expected_withdraw_amount,
        expected_spl_balance,
        slot_wait_index,
        error_code,
    } = params;
    // Given.
    //
    // A vesting account.
    let StartState {
        client,
        vesting_acc,
        vesting_acc_beneficiary,
        vesting_acc_slots,
        safe_acc,
        safe_srm_vault,
        safe_srm_vault_authority,
        srm_mint,
        ..
    } = start_state(test_type, vesting_slot_offsets, vesting_amounts);
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
    if let Some(slot_wait_index) = slot_wait_index {
        common::blockchain::pass_time(client.rpc(), vesting_acc_slots[slot_wait_index]);
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
        let r = client.withdraw_srm_with_signers(&signers, &accounts, expected_withdraw_amount);
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
        assert_eq_vec(vesting_acc.amounts, expected_vesting_amounts);
    }
}

// Alisasing LsrmMinted type here because we don't use the `lsrm` field in these
// tests and so we can avoid making another needless type.
type StartState = lifecycle::LsrmMinted;

fn start_state(
    test_type: TestType,
    vesting_slot_offsets: Vec<u64>,
    vesting_amounts: Vec<u64>,
) -> StartState {
    match test_type {
        TestType::LsrmMinted(lsrm_count) => {
            lifecycle::mint_lsrm(lsrm_count, vesting_slot_offsets, vesting_amounts)
        }
        TestType::Normal => {
            let lifecycle::Deposited {
                client,
                vesting_acc,
                vesting_acc_beneficiary,
                vesting_acc_slots,
                vesting_acc_amounts,
                safe_acc,
                safe_srm_vault,
                safe_srm_vault_authority,
                srm_mint,
                ..
            } = lifecycle::deposit_with_schedule(vesting_slot_offsets, vesting_amounts);
            // Dummy keypair to stuff into type. Not used.
            let lsrm_token_acc_owner = Keypair::generate(&mut OsRng);
            lifecycle::LsrmMinted {
                client,
                vesting_acc,
                vesting_acc_beneficiary,
                vesting_acc_slots,
                vesting_acc_amounts,
                safe_acc,
                safe_srm_vault,
                safe_srm_vault_authority,
                srm_mint,
                lsrm: vec![],
                lsrm_token_acc_owner,
            }
        }
    }
}

struct WithdrawTestParams {
    test_type: TestType,
    vesting_slot_offsets: Vec<u64>,
    vesting_amounts: Vec<u64>,
    expected_vesting_amounts: Vec<u64>,
    expected_withdraw_amount: u64,
    expected_spl_balance: u64,
    slot_wait_index: Option<usize>,
    error_code: Option<u32>,
}

enum TestType {
    Normal,
    LsrmMinted(usize),
}
