use rand::rngs::OsRng;
use serum_common::pack::Pack;
use serum_lockup::accounts::Safe;
use serum_lockup_client::*;
use solana_client_gen::prelude::*;
use solana_client_gen::solana_sdk::program_pack::Pack as TokenPack;
use solana_client_gen::solana_sdk::signature::{Keypair, Signer};

mod common;

#[test]
fn initialized() {
    // Given.
    let serum_common_tests::Genesis {
        client, srm_mint, ..
    } = serum_common_tests::genesis::<Client>();

    // When.
    //
    // I create the safe account and initialize it.
    let safe_authority = Keypair::generate(&mut OsRng);
    let InitializeResponse {
        safe,
        nonce,
        vault,
        vault_authority,
        whitelist,
        ..
    } = client
        .initialize(InitializeRequest {
            mint: srm_mint.pubkey(),
            authority: safe_authority.pubkey(),
        })
        .unwrap();

    // Then.
    //
    // The Safe should be setup.
    {
        let account = client
            .rpc()
            .get_account_with_commitment(&safe, CommitmentConfig::recent())
            .unwrap()
            .value
            .unwrap();
        let safe_acc = Safe::unpack(&account.data).unwrap();
        assert_eq!(&account.owner, client.program());
        assert_eq!(account.data.len(), Safe::default().size().unwrap() as usize);
        assert_eq!(safe_acc.authority, safe_authority.pubkey());
        assert_eq!(safe_acc.initialized, true);
        assert_eq!(safe_acc.nonce, nonce);
        assert_eq!(safe_acc.whitelist, whitelist);
    }
    // Then.
    //
    // The safe's SPL vault should be setup.
    {
        let account = client
            .rpc()
            .get_account_with_commitment(&vault, CommitmentConfig::recent())
            .unwrap()
            .value
            .unwrap();
        let safe_acc_vault = spl_token::state::Account::unpack(&account.data).unwrap();
        assert_eq!(safe_acc_vault.owner, vault_authority);
        assert_eq!(
            safe_acc_vault.state,
            spl_token::state::AccountState::Initialized
        );
        assert_eq!(safe_acc_vault.amount, 0);
        assert_eq!(safe_acc_vault.mint, srm_mint.pubkey());
    }
}
