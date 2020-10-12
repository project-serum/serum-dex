use rand::rngs::OsRng;
use serum_lockup_client::*;
use solana_client_gen::prelude::*;

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
    let InitializeResponse {
        safe,
        vault,
        vault_authority,
        whitelist,
        ..
    } = client
        .initialize(InitializeRequest {
            mint: srm_mint.pubkey(),
            authority: safe_authority.pubkey(),
        })
        .unwrap();

    Initialized {
        client,
        safe_acc: safe,
        safe_srm_vault: vault,
        safe_srm_vault_authority: vault_authority,
        safe_authority,
        depositor,
        depositor_balance_before,
        srm_mint,
        whitelist,
    }
}

pub struct Initialized {
    pub client: Client,
    pub safe_acc: Pubkey,
    pub safe_srm_vault: Pubkey,
    pub safe_srm_vault_authority: Pubkey,
    pub safe_authority: Keypair,
    pub depositor: Keypair,
    pub depositor_balance_before: u64,
    pub srm_mint: Keypair,
    pub whitelist: Pubkey,
}

pub fn deposit_with_schedule(
    deposit_amount: u64,
    end_ts_offset: i64,
    period_count: u64,
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

    let end_ts = end_ts_offset
        + client
            .rpc()
            .get_block_time(client.rpc().get_slot().unwrap())
            .unwrap();
    let (vesting_acc, vesting_acc_beneficiary) = {
        let vesting_acc_beneficiary = Keypair::generate(&mut OsRng);
        let resp = client
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

        (resp.vesting, vesting_acc_beneficiary)
    };

    Deposited {
        client,
        vesting_acc_beneficiary,
        vesting_acc: vesting_acc,
        safe_acc: safe_acc,
        safe_srm_vault,
        safe_srm_vault_authority,
        srm_mint,
        safe_authority,
        end_ts,
        period_count,
        deposit_amount,
    }
}

pub struct Deposited {
    pub client: Client,
    pub vesting_acc_beneficiary: Keypair,
    pub vesting_acc: Pubkey,
    pub safe_acc: Pubkey,
    pub safe_srm_vault: Pubkey,
    pub safe_srm_vault_authority: Pubkey,
    pub srm_mint: Keypair,
    pub safe_authority: Keypair,
    pub end_ts: i64,
    pub period_count: u64,
    pub deposit_amount: u64,
}
