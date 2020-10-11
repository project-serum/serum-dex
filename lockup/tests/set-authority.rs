use common::lifecycle::{self, Initialized};
use serum_lockup::accounts::Safe;
use serum_lockup_client::*;
use solana_client_gen::solana_sdk::pubkey::Pubkey;

mod common;

#[test]
fn set_authority() {
    // Given.
    //
    // An initialized safe.
    let Initialized {
        client,
        safe_acc,
        safe_authority,
        ..
    } = lifecycle::initialize();

    // When.
    //
    // I set the authority to someone new.
    let new_authority = Pubkey::new_rand();
    let _ = client
        .set_authority(SetAuthorityRequest {
            authority: &safe_authority,
            safe: safe_acc,
            new_authority,
        })
        .unwrap();

    // Then.
    //
    // The safe account should be updated.
    {
        let safe_acc: Safe = client.safe(&safe_acc).unwrap();
        assert_eq!(safe_acc.authority, new_authority);
    }
}

#[test]
fn set_authority_zero() {
    // Given.
    //
    // An initialized safe.
    let Initialized {
        client,
        safe_acc,
        safe_authority,
        ..
    } = lifecycle::initialize();

    // When.
    //
    // I set the authority to zero.
    let new_authority = Pubkey::new_from_array([0; 32]);
    let _ = client
        .set_authority(SetAuthorityRequest {
            authority: &safe_authority,
            safe: safe_acc,
            new_authority,
        })
        .unwrap();

    // Then.
    //
    // The safe account should be updated.
    {
        let safe_acc = client.safe(&safe_acc).unwrap();
        assert_eq!(safe_acc.authority, new_authority);
    }
}
