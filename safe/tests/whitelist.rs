use serum_safe::accounts::{SafeAccount, Whitelist};
use solana_client_gen::solana_sdk::commitment_config::CommitmentConfig;
use solana_client_gen::solana_sdk::instruction::AccountMeta;
use solana_client_gen::solana_sdk::signature::{Keypair, Signature, Signer};
use spl_token::pack::Pack;

use rand::rngs::OsRng;

mod common;

#[test]
fn whitelist_add() {
    // Given.
    //
    // An initialized safe.
    let common::lifecycle::Initialized {
        client,
        safe_account,
        safe_authority,
        safe_srm_vault,
        depositor,
        depositor_balance_before,
        srm_mint,
        ..
    } = common::lifecycle::initialize();
    // A program to whitelist.
    let program_to_whitelist = Keypair::generate(&mut OsRng).pubkey();

    // When.
    //
    // I add to the whitelist.
    let accounts = [
        AccountMeta::new(safe_authority.pubkey(), true),
        AccountMeta::new(safe_account.pubkey(), false),
    ];
    let signers = [&safe_authority, client.payer()];
    client
        .whitelist_add_with_signers(&signers, &accounts, program_to_whitelist)
        .unwrap();

    // Then.
    //
    // The whitelist is updated.
    let account = client
        .rpc()
        .get_account_with_commitment(&safe_account.pubkey(), CommitmentConfig::recent())
        .unwrap()
        .value
        .unwrap();
    let safe_account = SafeAccount::unpack_from_slice(&account.data).unwrap();

    let expected_whitelist = {
        let mut w = Whitelist::zeroed();
        w.push(program_to_whitelist);
        w
    };

    assert_eq!(safe_account.whitelist, expected_whitelist);
}

#[test]
fn whitelist_remove() {
    // Given.
    //
    // An initialized safe with a whitelist.
    let common::lifecycle::InitializedWithWhitelist {
        client,
        safe_account,
        safe_authority,
        safe_srm_vault,
        depositor,
        depositor_balance_before,
        whitelist,
        srm_mint,
    } = common::lifecycle::initialize_with_whitelist();
    let program_to_remove = whitelist.get_at(0);

    // When.
    //
    // I remove from the whitelist.
    let accounts = [
        AccountMeta::new(safe_authority.pubkey(), true),
        AccountMeta::new(safe_account.pubkey(), false),
    ];
    let signers = [&safe_authority, client.payer()];
    client
        .whitelist_delete_with_signers(&signers, &accounts, *program_to_remove)
        .unwrap();

    // Then.
    //
    // The whitelist is updated.
    let account = client
        .rpc()
        .get_account_with_commitment(&safe_account.pubkey(), CommitmentConfig::recent())
        .unwrap()
        .value
        .unwrap();
    let safe_account = SafeAccount::unpack_from_slice(&account.data).unwrap();

    assert_eq!(safe_account.whitelist, Whitelist::zeroed());
}
