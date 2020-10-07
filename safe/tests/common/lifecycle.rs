//! The lifecycle module defines common functions used in safe tests to bring
//! the program up to a certain state or point in time. For example, immediately
//! for every deposit test, we want to skip the boilerplate and have everything
//! already initialized.
//!
//! Each stage here builds on eachother. Genesis -> Initialization -> Deposit, etc.

use rand::rngs::OsRng;
use serum_safe::accounts::Vesting;
use serum_safe::client::{Client, InitializeResponse};
use solana_client_gen::solana_sdk;
use solana_client_gen::solana_sdk::commitment_config::CommitmentConfig;
use solana_client_gen::solana_sdk::instruction::AccountMeta;
use solana_client_gen::solana_sdk::pubkey::Pubkey;
use solana_client_gen::solana_sdk::signature::{Keypair, Signer};
use solana_client_gen::solana_sdk::sysvar;
use solana_sdk::program_pack::Pack as TokenPack;

// Sets up the initial on-chain state for a serum safe.
pub fn initialize() -> Initialized {
    let serum_common_tests::Genesis {
        client,
        srm_mint,
        god,
        god_balance_before,
        ..
    } = serum_common_tests::genesis::<Client>();

    let depositor = god;
    let depositor_balance_before = god_balance_before;

    // Initialize the safe authority.
    let safe_authority = Keypair::generate(&mut OsRng);

    // Initialize the Safe.
    let init_accs = [AccountMeta::new_readonly(
        solana_sdk::sysvar::rent::id(),
        false,
    )];
    let InitializeResponse {
        safe_acc,
        vault_acc,
        vault_acc_authority,
        whitelist,
        ..
    } = client
        .create_all_accounts_and_initialize(
            &init_accs,
            &srm_mint.pubkey(),
            &safe_authority.pubkey(),
        )
        .unwrap();

    // Ensure the safe_srm_vault has 0 SRM before the deposit.
    {
        let safe_srm_vault_spl_acc = {
            let account = client
                .rpc()
                .get_account_with_commitment(&vault_acc.pubkey(), CommitmentConfig::recent())
                .unwrap()
                .value
                .unwrap();
            spl_token::state::Account::unpack_from_slice(&account.data).unwrap()
        };
        assert_eq!(safe_srm_vault_spl_acc.mint, srm_mint.pubkey());
        assert_eq!(safe_srm_vault_spl_acc.owner, vault_acc_authority,);
        assert_eq!(safe_srm_vault_spl_acc.amount, 0);
    };

    Initialized {
        client,
        safe_acc,
        safe_srm_vault: vault_acc,
        safe_srm_vault_authority: vault_acc_authority,
        safe_authority,
        depositor,
        depositor_balance_before,
        srm_mint,
        whitelist,
    }
}

pub struct Initialized {
    pub client: Client,
    pub safe_acc: Keypair,
    pub safe_srm_vault: Keypair,
    pub safe_srm_vault_authority: Pubkey,
    pub safe_authority: Keypair,
    pub depositor: Keypair,
    pub depositor_balance_before: u64,
    pub srm_mint: Keypair,
    pub whitelist: Pubkey,
}

pub fn deposit_with_schedule(deposit_amount: u64, end_slot: u64, period_count: u64) -> Deposited {
    let Initialized {
        client,
        safe_acc,
        safe_srm_vault,
        safe_srm_vault_authority,
        depositor,
        srm_mint,
        safe_authority,
        ..
    } = initialize();

    let (vesting_acc, vesting_acc_beneficiary) = {
        let vesting_acc_beneficiary = Keypair::generate(&mut OsRng);
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

        (keypair, vesting_acc_beneficiary)
    };

    Deposited {
        client,
        vesting_acc_beneficiary,
        vesting_acc: vesting_acc.pubkey(),
        safe_acc: safe_acc.pubkey(),
        safe_srm_vault,
        safe_srm_vault_authority,
        srm_mint,
        safe_authority,
        end_slot,
        period_count,
        deposit_amount,
    }
}

pub struct Deposited {
    pub client: Client,
    pub vesting_acc_beneficiary: Keypair,
    pub vesting_acc: Pubkey,
    pub safe_acc: Pubkey,
    pub safe_srm_vault: Keypair,
    pub safe_srm_vault_authority: Pubkey,
    pub srm_mint: Keypair,
    pub safe_authority: Keypair,
    pub end_slot: u64,
    pub period_count: u64,
    pub deposit_amount: u64,
}
