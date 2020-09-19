//! The lifecycle module defines common functions used in safe tests to bring
//! the program up to a certain state. For example, immediately after
//! initialization, a deposit, etc. They are used to setup tests.

use crate::common;
use rand::rngs::OsRng;
use serum_safe::accounts::{SafeAccount, SrmVault, VestingAccount, Whitelist};
use serum_safe::client::{Client, ClientError, RequestOptions};
use serum_safe::error::{SafeError, SafeErrorCode};
use serum_safe::pack::DynPack;
use solana_client_gen::solana_sdk;
use solana_client_gen::solana_sdk::commitment_config::CommitmentConfig;
use solana_client_gen::solana_sdk::instruction::AccountMeta;
use solana_client_gen::solana_sdk::pubkey::Pubkey;
use solana_client_gen::solana_sdk::signature::{Keypair, Signature, Signer};
use solana_transaction_status::UiTransactionEncoding;
use spl_token::pack::Pack;
use std::error::Error;

pub const SPL_MINT_DECIMALS: u8 = 3;

pub fn genesis() -> Genesis {
    let client = common::client();

    // Setup.
    //
    // Initialize the SPL token representing SRM.
    let mint_authority = Keypair::from_bytes(&Keypair::to_bytes(client.payer().clone())).unwrap();
    let srm_mint = Keypair::generate(&mut OsRng);
    let _ = serum_common::rpc::create_and_init_mint(
        client.rpc(),
        client.payer(),
        &srm_mint,
        &mint_authority.pubkey(),
        3,
    )
    .unwrap();

    // Setup.
    //
    // Create a funded SRM SPL account representing the depositor allocating
    // vesting accounts.
    let god_balance_before = 1_000_000;
    let god = serum_common::rpc::mint_to_new_account(
        client.rpc(),
        client.payer(),
        &mint_authority,
        &srm_mint.pubkey(),
        god_balance_before,
    )
    .unwrap();

    Genesis {
        client,
        mint_authority,
        srm_mint,
        god,
        god_balance_before,
    }
}

// Genesis defines the initial state of the world.
pub struct Genesis {
    // RPC client.
    pub client: Client,
    // SRM mint authority.
    pub mint_authority: Keypair,
    // SRM.
    pub srm_mint: Keypair,
    // Account funded with a ton of SRM.
    pub god: Keypair,
    // Balance of the god account to start.
    pub god_balance_before: u64,
}

// Sets up the initial on-chain state for a serum safe.
pub fn initialize() -> Initialized {
    let Genesis {
        client,
        mint_authority,
        srm_mint,
        god,
        god_balance_before,
    } = genesis();

    let depositor = god;
    let depositor_balance_before = god_balance_before;

    // Initialize the safe authority.
    let safe_authority = Keypair::generate(&mut OsRng);

    // Initialize the Safe.
    let rent_account = AccountMeta::new_readonly(solana_sdk::sysvar::rent::id(), false);
    let init_accounts = vec![rent_account];
    let (_signature, safe_account) = client
        .create_account_and_initialize(&init_accounts, srm_mint.pubkey(), safe_authority.pubkey())
        .unwrap();

    // Create an SPL account representing the Safe program's vault.
    let safe_srm_vault = {
        let safe_srm_vault_program_derived_address =
            SrmVault::program_derived_address(client.program(), &safe_account.pubkey());
        let safe_srm_vault = serum_common::rpc::create_spl_account(
            client.rpc(),
            &srm_mint.pubkey(),
            &safe_srm_vault_program_derived_address,
            client.payer(),
        )
        .unwrap();

        // Ensure the safe_srm_vault has 0 SRM before the deposit.
        let safe_srm_vault_account = {
            let account = client
                .rpc()
                .get_account_with_commitment(&safe_srm_vault.pubkey(), CommitmentConfig::recent())
                .unwrap()
                .value
                .unwrap();
            spl_token::state::Account::unpack_from_slice(&account.data).unwrap()
        };
        assert_eq!(safe_srm_vault_account.mint, srm_mint.pubkey());
        assert_eq!(
            safe_srm_vault_account.owner,
            safe_srm_vault_program_derived_address
        );
        assert_eq!(safe_srm_vault_account.amount, 0);

        safe_srm_vault
    };
    Initialized {
        client,
        safe_account,
        safe_srm_vault,
        safe_authority,
        depositor,
        depositor_balance_before,
    }
}

pub struct Initialized {
    pub client: Client,
    pub safe_account: Keypair,
    pub safe_srm_vault: Keypair,
    pub safe_authority: Keypair,
    pub depositor: Keypair,
    pub depositor_balance_before: u64,
}

pub fn initialize_with_whitelist() -> InitializedWithWhitelist {
    // An initialized safe.
    let Initialized {
        client,
        safe_account,
        safe_authority,
        safe_srm_vault,
        depositor,
        depositor_balance_before,
    } = initialize();

    // A program to whitelist.
    let program_to_whitelist = Keypair::generate(&mut OsRng).pubkey();
    let whitelist = {
        let mut w = Whitelist::zeroed();
        w.push(program_to_whitelist);
        w
    };

    // Add to whitelist.
    let accounts = [
        AccountMeta::new(safe_authority.pubkey(), true),
        AccountMeta::new(safe_account.pubkey(), false),
    ];
    let signers = [&safe_authority, client.payer()];
    client
        .whitelist_add_with_signers(&signers, &accounts, program_to_whitelist)
        .unwrap();

    InitializedWithWhitelist {
        client,
        safe_account,
        safe_authority,
        safe_srm_vault,
        depositor,
        depositor_balance_before,
        whitelist,
    }
}

pub struct InitializedWithWhitelist {
    pub client: Client,
    pub safe_account: Keypair,
    pub safe_srm_vault: Keypair,
    pub safe_authority: Keypair,
    pub depositor: Keypair,
    pub depositor_balance_before: u64,
    pub whitelist: Whitelist,
}

pub fn deposit() -> Deposited {
    let Initialized {
        client,
        safe_account,
        safe_authority,
        safe_srm_vault,
        depositor,
        depositor_balance_before,
    } = initialize();

    let (vesting_account, vesting_account_beneficiary, expected_slots, expected_amounts) = {
        let deposit_accounts = [
            AccountMeta::new(depositor.pubkey(), false),
            AccountMeta::new(client.payer().pubkey(), true), // Owner of the depositor SPL account.
            AccountMeta::new(safe_srm_vault.pubkey(), false),
            AccountMeta::new(safe_account.pubkey(), false),
            AccountMeta::new(spl_token::ID, false),
            AccountMeta::new_readonly(solana_sdk::sysvar::rent::ID, false),
        ];
        let vesting_account_beneficiary = Keypair::generate(&mut OsRng);
        let vesting_slots = vec![11, 12, 13, 14, 15];
        let vesting_amounts = vec![1, 2, 3, 4, 5];
        let vesting_account_size = VestingAccount::data_size(vesting_slots.len());
        let (signature, keypair) = client
            .create_account_with_size_and_deposit_srm(
                vesting_account_size,
                &deposit_accounts,
                vesting_account_beneficiary.pubkey(),
                vesting_slots.clone(),
                vesting_amounts.clone(),
            )
            .unwrap();
        (
            keypair,
            vesting_account_beneficiary,
            vesting_slots,
            vesting_amounts,
        )
    };

    Deposited {
        client,
        vesting_account_beneficiary,
        vesting_account: vesting_account.pubkey(),
        safe_account: safe_account.pubkey(),
    }
}

pub struct Deposited {
    pub client: Client,
    pub vesting_account_beneficiary: Keypair,
    pub vesting_account: Pubkey,
    pub safe_account: Pubkey,
}
