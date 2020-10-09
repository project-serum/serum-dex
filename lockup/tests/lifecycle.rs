use common::blockchain;
use common::lifecycle::Initialized;
use rand::rngs::OsRng;
use serum_common::client::rpc;
use serum_common::pack::Pack;
use serum_lockup::accounts::{Vesting, Whitelist};
use serum_lockup_client::*;
use serum_lockup_test_stake::client::Client as StakeClient;
use solana_client_gen::prelude::*;
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
        vesting,
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

        let CreateVestingResponse {
            tx: _,
            vesting,
            mint,
        } = client
            .create_vesting(CreateVestingRequest {
                depositor: depositor.pubkey(),
                depositor_owner: client.payer(),
                safe: safe_acc,
                beneficiary: vesting_acc_beneficiary.pubkey(),
                end_slot,
                period_count,
                deposit_amount,
                mint_decimals: decimals,
            })
            .unwrap();
        (
            vesting,
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
                .get_account_with_commitment(&vesting, CommitmentConfig::recent())
                .unwrap()
                .value
                .unwrap();
            Vesting::unpack(&account.data).unwrap()
        };
        assert_eq!(vesting_acc.safe, safe_acc);
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
        let safe_vault_spl_acc = client.vault(&safe_acc).unwrap();
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
        let _ = client
            .whitelist_add(WhitelistAddRequest {
                authority: &safe_authority,
                safe: safe_acc,
                program: staking_program_id,
            })
            .unwrap();
        // Check it.
        let whitelist = client.whitelist(&safe_acc).unwrap();
        let mut expected = Whitelist::default();
        expected.push(staking_program_id);
        assert_eq!(whitelist, expected);
    }

    let stake_amount = 98;
    // Transfer funds from the safe to the whitelisted program.
    {
        // Instruction data to proxy to the whitelisted program.
        let relay_data = {
            let stake_instr = serum_lockup_test_stake::instruction::StakeInstruction::Stake {
                amount: stake_amount,
            };
            let mut relay_data = vec![0; stake_instr.size().unwrap() as usize];
            serum_lockup_test_stake::instruction::StakeInstruction::pack(stake_instr, &mut relay_data)
                .unwrap();

            relay_data
        };
        // Send tx.
        let _ = client.whitelist_withdraw(WhitelistWithdrawRequest {
            beneficiary: &expected_beneficiary,
            vesting,
            safe: safe_acc,
            whitelist_program: staking_program_id,
            vault: safe_srm_vault,
            whitelist_vault: stake_init.vault,
            whitelist_vault_authority: stake_init.vault_authority,
            delegate_amount: stake_amount,
            relay_data,
            relay_accounts: vec![AccountMeta::new(stake_init.instance, false)],
        });

        // Checks.
        {
            // Safe's vault should be decremented.
            let vault = client.vault(&safe_acc).unwrap();
            let expected_amount = expected_deposit - stake_amount;
            assert_eq!(vault.amount, expected_amount);
            assert_eq!(vault.delegated_amount, 0);
            assert_eq!(vault.delegate, COption::None);

            // Vesting account should be updated.
            let vesting = rpc::account_unpacked::<Vesting>(client.rpc(), &vesting);
            assert_eq!(vesting.whitelist_owned, stake_amount);

            // Staking program's vault should be incremented.
            let vault =
                rpc::account_token_unpacked::<TokenAccount>(client.rpc(), &stake_init.vault);
            assert_eq!(vault.amount, stake_amount);
        }
    }

    // Transfer funds from the whitelisted program back to the Safe.
    {
        let stake_withdraw = 95;
        // Relay tx data.
        let relay_data = {
            let stake_instr = serum_lockup_test_stake::instruction::StakeInstruction::Unstake {
                amount: stake_withdraw,
            };
            let mut relay_data = vec![0; stake_instr.size().unwrap() as usize];
            serum_lockup_test_stake::instruction::StakeInstruction::pack(stake_instr, &mut relay_data)
                .unwrap();
            relay_data
        };
        // Send tx.
        let _ = client.whitelist_deposit(WhitelistDepositRequest {
            beneficiary: &expected_beneficiary,
            vesting,
            safe: safe_acc,
            whitelist_program: staking_program_id,
            vault: safe_srm_vault,
            whitelist_vault: stake_init.vault,
            whitelist_vault_authority: stake_init.vault_authority,
            relay_data,
            relay_accounts: vec![AccountMeta::new(stake_init.instance, false)],
        });

        // Checks.
        {
            // Safe vault should be incremented.
            let vault = client.vault(&safe_acc).unwrap();
            assert_eq!(
                vault.amount,
                expected_deposit - stake_amount + stake_withdraw
            );

            // Vesting should be updated.
            let vesting = client.vesting(&vesting).unwrap();
            assert_eq!(vesting.whitelist_owned, stake_amount - stake_withdraw);

            // Stake vault should be decremented.
            let vault =
                rpc::account_token_unpacked::<TokenAccount>(client.rpc(), &stake_init.vault);
            assert_eq!(vault.amount, stake_amount - stake_withdraw);
        }
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
        let _ = client
            .claim(ClaimRequest {
                beneficiary: &expected_beneficiary,
                safe: safe_acc,
                vesting: vesting,
                locked_mint: nft_mint,
                locked_token_account: nft_tok_acc.pubkey(),
            })
            .unwrap();
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

        let redeem_amount = 10;
        let new_nft_amount = expected_deposit - redeem_amount;

        let _ = client
            .redeem(RedeemRequest {
                beneficiary: &expected_beneficiary,
                vesting,
                token_account: bene_tok_acc.pubkey(),
                vault: safe_srm_vault,
                safe: safe_acc,
                locked_token_account: nft_tok_acc.pubkey(),
                locked_mint: nft_mint,
                amount: redeem_amount,
            })
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
