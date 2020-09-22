use common::lifecycle::{self, LsrmMinted};
use solana_client_gen::solana_sdk::instruction::AccountMeta;

mod common;

#[test]
fn burn_lsrm() {
    // Given.
    //
    // A vesting account with outstanding lSRM.
    let LsrmMinted {
        client,
        vesting_account,
        vesting_account_beneficiary,
        srm_mint,
        vesting_account_slots,
        vesting_account_amounts,
        safe_account,
        safe_srm_vault,
        safe_srm_vault_authority,
        lsrm,
    } = lifecycle::mint_lsrm(2, vec![10_000, 20_000, 30_000], vec![10, 20, 30]);

    // When.
    //
    // I burn my lSRM.
    //    let lsrm_to_burn = lsrm[0];
    //    let accounts = [
    //				AccountMeta::new()
    //		];
    //client.burn_lsrm();

    // Then.
    //
    // My vesting account should be updated.

    // Then.
    //
    // My lSRM receipt should be burned.
}
