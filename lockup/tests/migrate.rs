use common::lifecycle;
use rand::rngs::OsRng;
use serum_lockup_client::*;
use solana_client_gen::prelude::*;
use solana_client_gen::solana_sdk::signature::{Keypair, Signer};

mod common;

// TODO: if we enable migration, then we need to check if migration has happened
//       on all instructions and exit if so.

#[test]
fn migrate() {
    // Given.
    //
    // An initialized safe with deposit (scheduled doesn't matter for
    // this test).
    let deposit_amount = 100;
    let lifecycle::Deposited {
        client,
        safe_acc,
        srm_mint,
        safe_authority,
        ..
    } = lifecycle::deposit_with_schedule(100, 100_000, 1);
    // And.
    //
    // An SPL account to transfer to.
    let recipient_owner = Keypair::generate(&mut OsRng);
    let receiver_token_acc = serum_common::client::rpc::create_token_account(
        client.rpc(),
        &srm_mint.pubkey(),
        &recipient_owner.pubkey(),
        client.payer(),
    )
    .unwrap();

    // When.
    //
    // I migrate the safe.
    let _ = client
        .migrate(MigrateRequest {
            authority: &safe_authority,
            safe: safe_acc,
            new_token_account: receiver_token_acc.pubkey(),
        })
        .unwrap();

    // Then.
    //
    // The safe's vault should be drained.
    {
        let safe_vault = client.vault(&safe_acc).unwrap();
        assert_eq!(safe_vault.amount, 0);
    }
    // Then.
    //
    // The receipient should have all the funds.
    {
        let recipient: spl_token::state::Account =
            serum_common::client::rpc::account_token_unpacked(
                client.rpc(),
                &receiver_token_acc.pubkey(),
            );
        assert_eq!(recipient.amount, deposit_amount);
    }

    // Then.
    //
    // The safe account should be updated.
    {
        // todo: add a check here if we want to place a marker for migration
        //       on the Safe state.
    }
}
