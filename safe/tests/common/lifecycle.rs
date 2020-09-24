//! The lifecycle module defines common functions used in safe tests to bring
//! the program up to a certain state or point in time. For example, immediately
//! for every deposit test, we want to skip the boilerplate and have everything
//! already initialized.
//!
//! Each stage here builds on eachother. Genesis -> Initialization -> Deposit, etc.

use crate::common;
use rand::rngs::OsRng;
use serum_safe::accounts::Vesting;
use serum_safe::client::{Client, ClientMint, SafeInitialization};
use solana_client_gen::solana_sdk;
use solana_client_gen::solana_sdk::commitment_config::CommitmentConfig;
use solana_client_gen::solana_sdk::instruction::AccountMeta;
use solana_client_gen::solana_sdk::pubkey::Pubkey;
use solana_client_gen::solana_sdk::signature::{Keypair, Signer};
use solana_client_gen::solana_sdk::sysvar;
use spl_token::pack::Pack as TokenPack;

pub fn genesis() -> Genesis {
    let client = common::client();

    let spl_mint_decimals = 3;

    // Setup.
    //
    // Initialize the SPL token representing SRM.
    let mint_authority = Keypair::from_bytes(&Keypair::to_bytes(client.payer().clone())).unwrap();
    let srm_mint = Keypair::generate(&mut OsRng);
    let _ = serum_common::client::rpc::create_and_init_mint(
        client.rpc(),
        client.payer(),
        &srm_mint,
        &mint_authority.pubkey(),
        spl_mint_decimals,
    )
    .unwrap();

    // Setup.
    //
    // Create a funded SRM SPL account representing the depositor allocating
    // vesting accounts.
    let god_balance_before = 1_000_000;
    let god = serum_common::client::rpc::mint_to_new_account(
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
        srm_mint,
        god,
        god_balance_before,
        ..
    } = genesis();

    let depositor = god;
    let depositor_balance_before = god_balance_before;

    // Initialize the safe authority.
    let safe_authority = Keypair::generate(&mut OsRng);

    // Initialize the Safe.
    let init_accs = [AccountMeta::new_readonly(
        solana_sdk::sysvar::rent::id(),
        false,
    )];
    let SafeInitialization {
        safe_acc,
        vault_acc,
        vault_acc_authority,
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
}

pub fn deposit() -> Deposited {
    let vesting_slots = vec![11, 12, 13, 14, 15];
    let vesting_amounts = vec![1, 2, 3, 4, 5];
    deposit_with_schedule(vesting_slots, vesting_amounts)
}

pub fn deposit_with_schedule(
    vesting_slot_offsets: Vec<u64>,
    vesting_amounts: Vec<u64>,
) -> Deposited {
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

    let (vesting_acc, vesting_acc_beneficiary, vesting_acc_slots, vesting_acc_amounts) = {
        let deposit_accs = [
            AccountMeta::new(depositor.pubkey(), false),
            // Authority of the depositing SPL account.
            AccountMeta::new(client.payer().pubkey(), true),
            AccountMeta::new(safe_srm_vault.pubkey(), false),
            AccountMeta::new(safe_acc.pubkey(), false),
            AccountMeta::new(spl_token::ID, false),
            AccountMeta::new_readonly(solana_sdk::sysvar::rent::ID, false),
        ];
        let current_slot = client.rpc().get_slot().unwrap();
        let vesting_slots = vesting_slot_offsets
            .iter()
            .map(|offset| current_slot + offset)
            .collect::<Vec<u64>>();
        let vesting_acc_beneficiary = Keypair::generate(&mut OsRng);
        let vesting_acc_size = Vesting::size_dyn(vesting_slots.len()).unwrap() as usize;
        let (_signature, keypair) = client
            .create_account_with_size_and_deposit(
                vesting_acc_size,
                &deposit_accs,
                vesting_acc_beneficiary.pubkey(),
                vesting_slots.clone(),
                vesting_amounts.clone(),
            )
            .unwrap();
        (
            keypair,
            vesting_acc_beneficiary,
            vesting_slots,
            vesting_amounts,
        )
    };

    Deposited {
        client,
        vesting_acc_beneficiary,
        vesting_acc: vesting_acc.pubkey(),
        vesting_acc_slots,
        vesting_acc_amounts,
        safe_acc: safe_acc.pubkey(),
        safe_srm_vault,
        safe_srm_vault_authority,
        srm_mint,
        safe_authority,
    }
}

pub struct Deposited {
    pub client: Client,
    pub vesting_acc_beneficiary: Keypair,
    pub vesting_acc: Pubkey,
    pub vesting_acc_slots: Vec<u64>,
    pub vesting_acc_amounts: Vec<u64>,
    pub safe_acc: Pubkey,
    pub safe_srm_vault: Keypair,
    pub safe_srm_vault_authority: Pubkey,
    pub srm_mint: Keypair,
    pub safe_authority: Keypair,
}

pub fn mint_lsrm(
    nft_count: usize,
    vesting_slot_offsets: Vec<u64>,
    vesting_amounts: Vec<u64>,
) -> LsrmMinted {
    let Deposited {
        client,
        vesting_acc,
        vesting_acc_beneficiary,
        vesting_acc_slots,
        vesting_acc_amounts,
        safe_acc,
        safe_srm_vault,
        safe_srm_vault_authority,
        srm_mint,
        ..
    } = deposit_with_schedule(vesting_slot_offsets, vesting_amounts);

    // Let the beneficiary be the owner for the NFTs.
    let lsrm_token_acc_owner = Keypair::from_bytes(&vesting_acc_beneficiary.to_bytes()).unwrap();

    let lsrm = {
        let mint_lsrm_accs = vec![
            AccountMeta::new(vesting_acc_beneficiary.pubkey(), true),
            AccountMeta::new(vesting_acc, false),
            AccountMeta::new_readonly(safe_acc, false),
            AccountMeta::new(safe_srm_vault_authority, false),
            AccountMeta::new(spl_token::ID, false),
            AccountMeta::new_readonly(sysvar::rent::ID, false),
        ];
        let signers = vec![&vesting_acc_beneficiary, client.payer()];
        let (_sig, lsrm_nfts) = client
            .create_nfts_and_mint_locked_with_signers(
                nft_count,
                &lsrm_token_acc_owner.pubkey(),
                signers,
                mint_lsrm_accs,
            )
            .unwrap();
        lsrm_nfts
    };

    // Sanity check we have 2 lSRM outstanding.
    {
        let vesting_acc: Vesting =
            serum_common::client::rpc::account_unpacked(client.rpc(), &vesting_acc);
        assert_eq!(vesting_acc.locked_outstanding, nft_count as u64);
    }

    LsrmMinted {
        client,
        vesting_acc,
        vesting_acc_beneficiary,
        srm_mint,
        vesting_acc_slots,
        vesting_acc_amounts,
        safe_acc,
        safe_srm_vault,
        safe_srm_vault_authority,
        lsrm,
        lsrm_token_acc_owner,
    }
}

pub struct LsrmMinted {
    pub client: Client,
    pub lsrm: Vec<ClientMint>,
    pub vesting_acc: Pubkey,
    pub vesting_acc_beneficiary: Keypair,
    pub vesting_acc_slots: Vec<u64>,
    pub vesting_acc_amounts: Vec<u64>,
    pub safe_acc: Pubkey,
    pub safe_srm_vault: Keypair,
    pub safe_srm_vault_authority: Pubkey,
    pub srm_mint: Keypair,
    // Authority/owner of all the token accounts holding lSRM NFTs.
    pub lsrm_token_acc_owner: Keypair,
}
