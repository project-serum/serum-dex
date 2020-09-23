use common::assert::assert_eq_vec;
use common::lifecycle::{self, LsrmMinted};
use serum_safe::accounts::{LsrmReceipt, VestingAccount};
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
        lsrm_token_account_owner,
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
        AccountMeta::new(lsrm_token_account_owner.pubkey(), true),
        AccountMeta::new(lsrm1.token_account.pubkey(), false),
        AccountMeta::new(lsrm1.mint.pubkey(), false),
        AccountMeta::new(lsrm1.receipt, false),
        AccountMeta::new(vesting_account, false),
        AccountMeta::new_readonly(spl_token::ID, false),
    ];
    let signers = &[&lsrm_token_account_owner, client.payer()];
    client
        .burn_locked_srm_with_signers(signers, accounts)
        .unwrap();

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
    {
        let receipt: LsrmReceipt =
            serum_common::client::rpc::account_unpacked(client.rpc(), &lsrm1.receipt);
        assert_eq!(receipt.burned, true);
    }
    // Then.
    //
    // I should no longer have lSRM in my account.
    {
        let account: spl_token::state::Account = serum_common::client::rpc::account_unpacked(
            client.rpc(),
            &lsrm1.token_account.pubkey(),
        );
        assert_eq!(account.amount, 0);
    }
    // Then.
    //
    // The NFT mint supply should be zero.
    {
        let mint: spl_token::state::Mint =
            serum_common::client::rpc::account_unpacked(client.rpc(), &lsrm1.mint.pubkey());
        assert_eq!(mint.supply, 0);
    }
}
