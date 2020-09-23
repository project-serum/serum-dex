use rand::rngs::OsRng;
use serum_safe::accounts::SafeAccount;
use serum_safe::client::SafeInitialization;
use serum_safe::error::SafeErrorCode;
use solana_client_gen::solana_sdk;
use solana_client_gen::solana_sdk::commitment_config::CommitmentConfig;
use solana_client_gen::solana_sdk::instruction::AccountMeta;
use solana_client_gen::solana_sdk::signature::{Keypair, Signer};
use spl_token::pack::Pack;

mod common;

#[test]
fn initialized() {
    // Given.
    let common::lifecycle::Genesis {
        client, srm_mint, ..
    } = common::lifecycle::genesis();

    // When.
    //
    // I create the safe account and initialize it.
    let safe_authority = Keypair::generate(&mut OsRng);
    let rent_account = AccountMeta::new_readonly(solana_sdk::sysvar::rent::id(), false);
    let accounts = vec![rent_account];
    let SafeInitialization {
        safe_account,
        nonce,
        vault_account,
        vault_account_authority,
        ..
    } = client
        .create_all_accounts_and_initialize(&accounts, &srm_mint.pubkey(), &safe_authority.pubkey())
        .unwrap();

    // Then.
    //
    // The SafeAccount should be setup.
    {
        let account = client
            .rpc()
            .get_account_with_commitment(&safe_account.pubkey(), CommitmentConfig::recent())
            .unwrap()
            .value
            .unwrap();
        let safe_account = SafeAccount::unpack_from_slice(&account.data).unwrap();
        assert_eq!(&account.owner, client.program());
        assert_eq!(account.data.len(), SafeAccount::SIZE);
        assert_eq!(safe_account.authority, safe_authority.pubkey());
        assert_eq!(safe_account.supply, 0);
        assert_eq!(safe_account.is_initialized, true);
        assert_eq!(safe_account.nonce, nonce);
    }
    // Then.
    //
    // The safe's SPL vault should be setup.
    {
        let account = client
            .rpc()
            .get_account_with_commitment(&vault_account.pubkey(), CommitmentConfig::recent())
            .unwrap()
            .value
            .unwrap();
        let safe_account_vault = spl_token::state::Account::unpack(&account.data).unwrap();
        assert_eq!(safe_account_vault.owner, vault_account_authority);
        assert_eq!(
            safe_account_vault.state,
            spl_token::state::AccountState::Initialized
        );
        assert_eq!(safe_account_vault.amount, 0);
        assert_eq!(safe_account_vault.mint, srm_mint.pubkey());
    }
}

#[test]
fn initialized_already() {
    // Given.
    let common::lifecycle::Genesis {
        client, srm_mint, ..
    } = common::lifecycle::genesis();

    // When
    //
    // I create the safe account and initialize it.
    let safe_authority = Keypair::generate(&mut OsRng);
    let rent_account = AccountMeta::new_readonly(solana_sdk::sysvar::rent::id(), false);
    let accounts = vec![rent_account.clone()];
    let SafeInitialization {
        safe_account,
        nonce,
        ..
    } = client
        .create_all_accounts_and_initialize(
            &accounts.clone(),
            &srm_mint.pubkey(),
            &safe_authority.pubkey(),
        )
        .unwrap();
    // And
    //
    // I try to initialize it for second time.
    let accounts_again = vec![AccountMeta::new(safe_account.pubkey(), false), rent_account];
    let new_safe_authority = Keypair::generate(&mut OsRng);
    match client.initialize(
        &accounts_again,
        srm_mint.pubkey(),
        new_safe_authority.pubkey(),
        nonce,
    ) {
        Ok(_) => panic!("transaction should fail"),
        Err(e) => {
            // Then.
            //
            // It should error.
            let expected: u32 = SafeErrorCode::AlreadyInitialized.into();
            assert_eq!(e.error_code().unwrap(), expected);
        }
    };
}
