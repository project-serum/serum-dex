use common::assert::assert_eq_vec;
use common::lifecycle::{self, LsrmMinted};
use serum_safe::accounts::VestingAccount;
use solana_client_gen::solana_sdk::instruction::AccountMeta;
use solana_client_gen::solana_sdk::signature::Signer;

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
    // Sanity check we have 2 lSRM outstanding (this isn't necessary).
    {
        let vesting_account: VestingAccount =
            serum_common::client::rpc::account_dyn_unpacked(client.rpc(), &vesting_account);
        assert_eq!(vesting_account.locked_outstanding, 2);
    }

    let lsrm1 = &lsrm[0];
    let lsrm2 = &lsrm[1];

    // When.
    //
    // I burn my lSRM.
    let accounts = &[
        AccountMeta::new(lsrm1.token_account.pubkey(), true),
        AccountMeta::new(lsrm1.mint.pubkey(), false),
        AccountMeta::new(vesting_account, false),
        AccountMeta::new(lsrm1.receipt, false),
    ];
    let signers = &[&lsrm1.token_account, client.payer()];
    client.burn_locked_srm_with_signers(signers, accounts);

    // Then.
    //
    // My vesting account should be updated.
    {
        let vesting_account: VestingAccount =
            serum_common::client::rpc::account_dyn_unpacked(client.rpc(), &vesting_account);
        assert_eq!(vesting_account.locked_outstanding, 1);
    }

    // Then.
    //
    // My lSRM receipt should be burned.

    // Then.
    //
    // I should no longer have lSRM in my account.

    // Then.
    //
    // The NFT mint supply should be zero.
}
