use common::lifecycle;
use rand::rngs::OsRng;
use solana_client_gen::solana_sdk::instruction::AccountMeta;
use solana_client_gen::solana_sdk::signature::{Keypair, Signer};

mod common;

// TODO: if we enable migration, then we need to check if migration has happened
//       on all instructions and exit if so.

#[test]
fn migrate() {
    // Given.
    //
    // An initialized safe with deposit.
    let lifecycle::Deposited {
        client,
        safe_account,
        safe_srm_vault,
        safe_srm_vault_authority,
        srm_mint,
        safe_authority,
				..
    } = lifecycle::deposit_with_schedule(vec![1, 2, 3, 4, 5], vec![100, 200, 300, 400, 500]);
    // And.
    //
    // An SPL account to transfer to.
    let recipient_owner = Keypair::generate(&mut OsRng);
    let receiver_spl_account = serum_common::client::rpc::create_spl_account(
        client.rpc(),
        &srm_mint.pubkey(),
        &recipient_owner.pubkey(),
        client.payer(),
    )
    .unwrap();

    // When.
    //
    // I migrate thh safe.
    let accounts = [
        AccountMeta::new_readonly(safe_authority.pubkey(), true),
        AccountMeta::new(safe_account, false),
        AccountMeta::new(safe_srm_vault.pubkey(), false),
        AccountMeta::new_readonly(safe_srm_vault_authority, false),
        AccountMeta::new(receiver_spl_account.pubkey(), false),
        AccountMeta::new_readonly(spl_token::ID, false),
    ];
    let signers = [&safe_authority, client.payer()];
    client.migrate_with_signers(&signers, &accounts).unwrap();

    // Then.
    //
    // The safe's vault should be drained.
    {
        let safe_vault: spl_token::state::Account =
            serum_common::client::rpc::account_unpacked(client.rpc(), &safe_srm_vault.pubkey());
        assert_eq!(safe_vault.amount, 0);
    }
    // Then.
    //
    // The receipient should have all the funds.
    {
        let recipient: spl_token::state::Account = serum_common::client::rpc::account_unpacked(
            client.rpc(),
            &receiver_spl_account.pubkey(),
        );
        assert_eq!(recipient.amount, 100 + 200 + 300 + 400 + 500);
    }

    // Then.
    //
    // The safe account should be updated.
    {
        // todo: add a check here if we want to place a marker for migration
        //       on the SafeAccount state.
    }
}
