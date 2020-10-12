use common::blockchain;
use common::lifecycle::Initialized;
use rand::rngs::OsRng;
use serum_common::client::rpc;
use serum_common::pack::Pack;
use serum_lockup::accounts::{Vesting, Whitelist, WhitelistEntry};
use serum_lockup_client::*;
use serum_lockup_test_stake::client::Client as StakeClient;
use solana_client_gen::prelude::*;
use solana_client_gen::solana_sdk::instruction::AccountMeta;
use solana_client_gen::solana_sdk::program_option::COption;
use solana_client_gen::solana_sdk::pubkey::Pubkey;
use solana_client_gen::solana_sdk::signature::{Keypair, Signer};
use spl_token::state::{Account as TokenAccount, Mint};

mod common;

#[test]
fn lifecycle() {
    let Initialized {
        client,
        safe_acc,
        srm_mint,
        safe_authority,
        safe_srm_vault_authority,
        depositor,
        depositor_balance_before,
        ..
    } = common::lifecycle::initialize();

    // CreateVesting.
    let (vesting, vesting_acc, expected_beneficiary, expected_deposit, nft_mint) = {
        let vesting_acc_beneficiary = Keypair::generate(&mut OsRng);
        let current_ts = client
            .rpc()
            .get_block_time(client.rpc().get_slot().unwrap())
            .unwrap();
        let end_ts = {
            let end_ts_offset = 100;
            end_ts_offset + current_ts
        };
        let period_count = 10;
        let deposit_amount = 100;
        // When.
        //
        // A depositor performs the vesting account deposit.
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
                end_ts,
                period_count,
                deposit_amount,
            })
            .unwrap();

        // Then.
        //
        // The vesting account is setup properly.
        let vesting_acc = client.vesting(&vesting).unwrap();
        assert_eq!(vesting_acc.safe, safe_acc);
        assert_eq!(vesting_acc.beneficiary, vesting_acc_beneficiary.pubkey());
        assert_eq!(vesting_acc.initialized, true);
        assert_eq!(vesting_acc.end_ts, end_ts);
        assert_eq!(vesting_acc.period_count, period_count);
        assert_eq!(vesting_acc.locked_nft_mint, mint);
        assert_eq!(vesting_acc.whitelist_owned, 0);
        // Then.
        //
        // The depositor's SPL token account has funds reduced.
        let depositor_spl_acc: spl_token::state::Account =
            rpc::account_token_unpacked(client.rpc(), &depositor.pubkey());
        let expected_balance = depositor_balance_before - deposit_amount;
        assert_eq!(depositor_spl_acc.amount, expected_balance);
        // Then.
        //
        // The program-owned SPL token vault has funds increased.
        let safe_vault_spl_acc = client.vault(&safe_acc).unwrap();
        assert_eq!(safe_vault_spl_acc.amount, deposit_amount);
        // Sanity check the owner of the vault account.
        assert_eq!(safe_vault_spl_acc.owner, safe_srm_vault_authority);
        (
            vesting,
            vesting_acc,
            vesting_acc_beneficiary,
            deposit_amount,
            mint,
        )
    };

    // Claim the vesting account.
    let nft_tok_acc = {
        let claim_resp = client
            .claim(ClaimRequest {
                beneficiary: &expected_beneficiary,
                safe: safe_acc,
                vesting: vesting,
            })
            .unwrap();
        let nft = rpc::account_token_unpacked::<TokenAccount>(
            client.rpc(),
            &claim_resp.locked_token_account,
        );
        assert_eq!(nft.amount, expected_deposit);
        claim_resp.locked_token_account
    };

    // Setup the staking program.
    let staking_program_id: Pubkey = std::env::var("TEST_WHITELIST_PROGRAM_ID")
        .unwrap()
        .parse()
        .unwrap();
    let stake_client = serum_common_tests::client_at::<StakeClient>(staking_program_id);
    let stake_init = stake_client.init(&srm_mint.pubkey()).unwrap();

    // Add it to whitelist.
    {
        let entry = WhitelistEntry::new(staking_program_id, stake_init.instance, stake_init.nonce);
        let _ = client
            .whitelist_add(WhitelistAddRequest {
                authority: &safe_authority,
                safe: safe_acc,
                entry: entry.clone(),
            })
            .unwrap();
        // Check it.
        client
            .with_whitelist(&safe_acc, |wl: Whitelist| {
                assert_eq!(wl.get_at(0).unwrap(), entry);
                for k in 1..Whitelist::LEN {
                    assert_eq!(wl.get_at(k).unwrap(), WhitelistEntry::zero());
                }
            })
            .unwrap();
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
            serum_lockup_test_stake::instruction::StakeInstruction::pack(
                stake_instr,
                &mut relay_data,
            )
            .unwrap();

            relay_data
        };
        // Send tx.
        let _ = client.whitelist_withdraw(WhitelistWithdrawRequest {
            beneficiary: &expected_beneficiary,
            vesting,
            safe: safe_acc,
            whitelist_program: staking_program_id,
            whitelist_program_vault: stake_init.vault,
            whitelist_program_vault_authority: stake_init.vault_authority,
            delegate_amount: stake_amount,
            relay_data,
            relay_accounts: vec![AccountMeta::new(stake_init.instance, false)],
            relay_signers: vec![],
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
            serum_lockup_test_stake::instruction::StakeInstruction::pack(
                stake_instr,
                &mut relay_data,
            )
            .unwrap();
            relay_data
        };
        // Send tx.
        let _ = client.whitelist_deposit(WhitelistDepositRequest {
            beneficiary: &expected_beneficiary,
            vesting,
            safe: safe_acc,
            whitelist_program: staking_program_id,
            whitelist_program_vault: stake_init.vault,
            whitelist_program_vault_authority: stake_init.vault_authority,
            relay_data,
            relay_accounts: vec![AccountMeta::new(stake_init.instance, false)],
            relay_signers: vec![],
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

    // Wait for a vesting period to lapse.
    {
        let wait_ts = vesting_acc.start_ts + 10;
        blockchain::pass_time(client.rpc(), wait_ts);
    }

    // Redeem 10 SRM.
    //
    // Current state:
    //
    // * original-deposit-amount 100
    // * balance: 97
    // * stake-amount/whitelist_owned: 3
    // * vested-amount: ~10 (depends on variance in ts time as tests run, this
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
                safe: safe_acc,
                amount: redeem_amount,
            })
            .unwrap();

        // The nft should be burnt for the redeem_amount.
        let nft = rpc::account_token_unpacked::<TokenAccount>(client.rpc(), &nft_tok_acc);
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
