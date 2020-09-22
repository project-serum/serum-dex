use common::lifecycle::{self, Initialized};
use rand::rngs::OsRng;
use serum_safe::accounts::SafeAccount;
use solana_client_gen::solana_sdk::commitment_config::CommitmentConfig;
use solana_client_gen::solana_sdk::instruction::AccountMeta;
use solana_client_gen::solana_sdk::pubkey::Pubkey;
use solana_client_gen::solana_sdk::signature::{Keypair, Signer};
use std::str::FromStr;

mod common;

#[test]
fn set_authority() {
    // Given.
    //
    // An initialized safe.
    let Initialized {
        client,
        safe_account,
        safe_srm_vault,
        safe_srm_vault_authority,
        safe_authority,
        depositor,
        depositor_balance_before,
        srm_mint,
    } = lifecycle::initialize();

    // When.
    //
    // I set the authority to someone new.
    let new_authority = Keypair::generate(&mut OsRng);
    let accounts = [
        AccountMeta::new_readonly(safe_authority.pubkey(), true),
        AccountMeta::new(safe_account.pubkey(), false),
    ];
    let signers = [&safe_authority, client.payer()];
    client
        .set_authority_with_signers(&signers, &accounts, new_authority.pubkey())
        .unwrap();

    // Then.
    //
    // The safe account should be updated.
    {
        let safe_account: SafeAccount =
            serum_common_client::rpc::account_unpacked(client.rpc(), &safe_account.pubkey());
        assert_eq!(safe_account.authority, new_authority.pubkey());
    }
}

#[test]
fn set_authority_zero() {
    // Given.
    //
    // An initialized safe.
    let Initialized {
        client,
        safe_account,
        safe_srm_vault,
        safe_srm_vault_authority,
        safe_authority,
        depositor,
        depositor_balance_before,
        srm_mint,
    } = lifecycle::initialize();

    // When.
    //
    // I set the authority to zero.
    let new_authority = Pubkey::new_from_array([0; 32]);
    let accounts = [
        AccountMeta::new_readonly(safe_authority.pubkey(), true),
        AccountMeta::new(safe_account.pubkey(), false),
    ];
    let signers = [&safe_authority, client.payer()];
    client
        .set_authority_with_signers(&signers, &accounts, new_authority)
        .unwrap();

    // Then.
    //
    // The safe account should be updated.
    {
        let safe_account: SafeAccount =
            serum_common_client::rpc::account_unpacked(client.rpc(), &safe_account.pubkey());
        assert_eq!(safe_account.authority, new_authority);
    }
}
