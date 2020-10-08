use common::blockchain;
use common::lifecycle::Initialized;
use rand::rngs::OsRng;
use serum_common::client::rpc;
use serum_common::pack::Pack;
use serum_safe::accounts::{Vesting, Whitelist};
use serum_wl_program::client::Client as StakeClient;
use solana_client_gen::solana_sdk;
use solana_client_gen::solana_sdk::commitment_config::CommitmentConfig;
use solana_client_gen::solana_sdk::instruction::AccountMeta;
use solana_client_gen::solana_sdk::program_option::COption;
use solana_client_gen::solana_sdk::pubkey::Pubkey;
use solana_client_gen::solana_sdk::signature::{Keypair, Signer};
use spl_token::state::{Account as TokenAccount, Mint};

mod common;

#[test]
fn lifecycle() {
    // Given.
    //
    // An initialized safe.
    let Initialized {
        client,
        safe_acc,
        srm_mint,
        safe_authority,
        safe_srm_vault,
        safe_srm_vault_authority,
        depositor,
        depositor_balance_before,
        whitelist,
        ..
    } = common::lifecycle::initialize();

    // When.
    //
    // A depositor performs the vesting account deposit.
    let (
        vesting_acc_kp,
        expected_beneficiary,
        expected_deposit,
        expected_end_slot,
        expected_period_count,
        nft_mint,
    ) = {
        let vesting_acc_beneficiary = Keypair::generate(&mut OsRng);
        let current_slot = client.rpc().get_slot().unwrap();
        let end_slot = {
            let end_slot_offset = 100;
            end_slot_offset + current_slot
        };
        let period_count = 10;
        let deposit_amount = 100;
        let decimals = 3;

        let (_signature, keypair, mint) = client
            .create_vesting_account(
                &depositor.pubkey(),
                &safe_acc.pubkey(),
                &safe_srm_vault.pubkey(),
                &safe_srm_vault_authority,
                &vesting_acc_beneficiary.pubkey(),
                end_slot,
                period_count,
                deposit_amount,
                decimals,
            )
            .unwrap();
        (
            keypair,
            vesting_acc_beneficiary,
            deposit_amount,
            end_slot,
            period_count,
            mint,
        )
    };

    // Then.
    //
    // The vesting account is setup properly.
    let vesting_acc = {
        let vesting_acc = {
            let account = client
                .rpc()
                .get_account_with_commitment(&vesting_acc_kp.pubkey(), CommitmentConfig::recent())
                .unwrap()
                .value
                .unwrap();
            Vesting::unpack(&account.data).unwrap()
        };
        assert_eq!(vesting_acc.safe, safe_acc.pubkey());
        assert_eq!(vesting_acc.beneficiary, expected_beneficiary.pubkey());
        assert_eq!(vesting_acc.initialized, true);
        assert_eq!(vesting_acc.end_slot, expected_end_slot);
        assert_eq!(vesting_acc.period_count, expected_period_count);
        assert_eq!(vesting_acc.locked_nft_mint, nft_mint);
        assert_eq!(vesting_acc.whitelist_owned, 0);
        vesting_acc
    };
    // Then.
    //
    // The depositor's SPL token account has funds reduced.
    {
        let depositor_spl_acc: spl_token::state::Account =
            rpc::account_token_unpacked(client.rpc(), &depositor.pubkey());
        let expected_balance = depositor_balance_before - expected_deposit;
        assert_eq!(depositor_spl_acc.amount, expected_balance);
    }
    // Then.
    //
    // The program-owned SPL token vault has funds increased.
    {
        let safe_vault_spl_acc: spl_token::state::Account =
            rpc::account_token_unpacked(client.rpc(), &safe_srm_vault.pubkey());
        assert_eq!(safe_vault_spl_acc.amount, expected_deposit);
        // Sanity check the owner of the vault account.
        assert_eq!(safe_vault_spl_acc.owner, safe_srm_vault_authority);
    }

    // Setup the staking program.
    let staking_program_id: Pubkey = std::env::var("TEST_WHITELIST_PROGRAM_ID")
        .unwrap()
        .parse()
        .unwrap();
    let stake_client = serum_common_tests::client_at::<StakeClient>(staking_program_id);
    let stake_init = stake_client.init(&srm_mint.pubkey()).unwrap();

    // Add it to whitelist.
    {
        let accounts = [
            AccountMeta::new_readonly(safe_authority.pubkey(), true),
            AccountMeta::new_readonly(safe_acc.pubkey(), false),
            AccountMeta::new(whitelist, false),
        ];
        let signers = [client.payer(), &safe_authority];
        client
            .whitelist_add_with_signers(&signers, &accounts, staking_program_id)
            .unwrap();

        let whitelist = rpc::account_unpacked::<Whitelist>(client.rpc(), &whitelist);

        let mut expected = Whitelist::default();
        expected.push(staking_program_id);

        assert_eq!(whitelist, expected);
    }

    // Accounts for the next two transacitons.
    let mut accounts = vec![
        AccountMeta::new_readonly(expected_beneficiary.pubkey(), true),
        AccountMeta::new(vesting_acc_kp.pubkey(), false),
        AccountMeta::new_readonly(safe_acc.pubkey(), false),
        AccountMeta::new_readonly(safe_srm_vault_authority, false),
        AccountMeta::new_readonly(staking_program_id, false),
        // Below are relay accounts.
        AccountMeta::new(safe_srm_vault.pubkey(), false),
        AccountMeta::new(stake_init.vault, false),
        AccountMeta::new_readonly(stake_init.vault_authority, false),
        AccountMeta::new_readonly(spl_token::ID, false),
        // Program specific relay accounts.
        AccountMeta::new(stake_init.instance, false),
    ];
    let stake_amount = 98;
    // Transfer funds from the safe to the whitelisted program.
    {
        let stake_instr = serum_wl_program::instruction::WlInstruction::Stake {
            amount: stake_amount,
        };
        let signers = [client.payer(), &expected_beneficiary];
        let mut relay_data = vec![0; stake_instr.size().unwrap() as usize];
        serum_wl_program::instruction::WlInstruction::pack(stake_instr, &mut relay_data).unwrap();

        let _tx_sig = client
            .whitelist_withdraw_with_signers(&signers, &accounts, stake_amount, relay_data)
            .unwrap();

        // Safe's vault should be decremented.
        let vault =
            rpc::account_token_unpacked::<TokenAccount>(client.rpc(), &safe_srm_vault.pubkey());
        let expected_amount = expected_deposit - stake_amount;
        assert_eq!(vault.amount, expected_amount);
        assert_eq!(vault.delegated_amount, 0);
        assert_eq!(vault.delegate, COption::None);

        // Vesting account should be updated.
        let vesting = rpc::account_unpacked::<Vesting>(client.rpc(), &vesting_acc_kp.pubkey());
        assert_eq!(vesting.whitelist_owned, stake_amount);

        // Staking program's vault should be incremented.
        let vault = rpc::account_token_unpacked::<TokenAccount>(client.rpc(), &stake_init.vault);
        assert_eq!(vault.amount, stake_amount);
    }

    // Transfer funds from the whitelisted program back to the Safe.
    {
        let stake_withdraw = 95;
        let stake_instr = serum_wl_program::instruction::WlInstruction::Unstake {
            amount: stake_withdraw,
        };
        let signers = [client.payer(), &expected_beneficiary];
        let mut relay_data = vec![0; stake_instr.size().unwrap() as usize];
        serum_wl_program::instruction::WlInstruction::pack(stake_instr, &mut relay_data).unwrap();
        let _tx_sig = client
            .whitelist_deposit_with_signers(&signers, &accounts, relay_data)
            .unwrap();

        // Safe vault should be incremented.
        let vault =
            rpc::account_token_unpacked::<TokenAccount>(client.rpc(), &safe_srm_vault.pubkey());
        assert_eq!(
            vault.amount,
            expected_deposit - stake_amount + stake_withdraw
        );

        // Vesting should be updated.
        let vesting = rpc::account_unpacked::<Vesting>(client.rpc(), &vesting_acc_kp.pubkey());
        assert_eq!(vesting.whitelist_owned, stake_amount - stake_withdraw);

        // Stake vault should be decremented.
        let vault = rpc::account_token_unpacked::<TokenAccount>(client.rpc(), &stake_init.vault);
        assert_eq!(vault.amount, stake_amount - stake_withdraw);
    }

    let nft_tok_acc = rpc::create_token_account(
        client.rpc(),
        &nft_mint,
        &expected_beneficiary.pubkey(),
        client.payer(),
    )
    .unwrap();

    // Claim.
    {
        let accounts = [
            AccountMeta::new_readonly(expected_beneficiary.pubkey(), true),
            AccountMeta::new(vesting_acc_kp.pubkey(), false),
            AccountMeta::new_readonly(safe_acc.pubkey(), false),
            AccountMeta::new_readonly(safe_srm_vault_authority, false),
            AccountMeta::new_readonly(spl_token::ID, false),
            AccountMeta::new_readonly(solana_sdk::sysvar::rent::ID, false),
            AccountMeta::new(nft_mint, false),
            AccountMeta::new(nft_tok_acc.pubkey(), false),
        ];
        let signers = [client.payer(), &expected_beneficiary];
        let _ = client.claim_with_signers(&signers, &accounts).unwrap();

        let nft = rpc::account_token_unpacked::<TokenAccount>(client.rpc(), &nft_tok_acc.pubkey());
        assert_eq!(nft.amount, expected_deposit);
    }

    // Wait for a vesting period to lapse.
    {
        let wait_slot = vesting_acc.start_slot + 10;
        blockchain::pass_time(client.rpc(), wait_slot);
    }

    // Redeem 10 SRM.
    //
    // Current state:
    //
    // * original-deposit-amount 100
    // * balance: 97
    // * stake-amount/whitelist_owned: 3
    // * vested-amount: ~10 (depends on variance in slot time as tests run, this
    //                       is a lower bound.)
    {
        let bene_tok_acc = rpc::create_token_account(
            client.rpc(),
            &srm_mint.pubkey(),
            &expected_beneficiary.pubkey(),
            client.payer(),
        )
        .unwrap();
        let accounts = [
            AccountMeta::new_readonly(expected_beneficiary.pubkey(), true),
            AccountMeta::new(vesting_acc_kp.pubkey(), false),
            AccountMeta::new(bene_tok_acc.pubkey(), false),
            AccountMeta::new(safe_srm_vault.pubkey(), false),
            AccountMeta::new_readonly(safe_srm_vault_authority, false),
            AccountMeta::new_readonly(safe_acc.pubkey(), false),
            AccountMeta::new(nft_tok_acc.pubkey(), false),
            AccountMeta::new(nft_mint, false),
            AccountMeta::new_readonly(spl_token::ID, false),
            AccountMeta::new_readonly(solana_sdk::sysvar::clock::ID, false),
        ];
        let signers = [client.payer(), &expected_beneficiary];
        let redeem_amount = 10;
        let new_nft_amount = expected_deposit - redeem_amount;
        client
            .redeem_with_signers(&signers, &accounts, redeem_amount)
            .unwrap();
        // The nft should be burnt for the redeem_amount.
        let nft = rpc::account_token_unpacked::<TokenAccount>(client.rpc(), &nft_tok_acc.pubkey());
        assert_eq!(nft.amount, new_nft_amount);

        // The supply should be burnt.
        let nft_mint = rpc::account_token_unpacked::<Mint>(client.rpc(), &nft_mint);
        assert_eq!(nft_mint.supply, new_nft_amount);

        // The SRM account should be increased.
        let bene_tok =
            rpc::account_token_unpacked::<TokenAccount>(client.rpc(), &bene_tok_acc.pubkey());
        assert_eq!(bene_tok.amount, redeem_amount);
    }
}
