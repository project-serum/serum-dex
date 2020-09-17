use serum_common::Cluster;
use serum_safe::client::{Client, RequestOptions};
use solana_client::rpc_config::RpcSendTransactionConfig;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "srm-safe", about = "A cli to interact with the Serum Safe")]
struct Opt {
    /// Program id on Solana.
    #[structopt(long)]
    program_id: Pubkey,

    /// Cluster identifier to communicate with.
    #[structopt(long)]
    cluster: Cluster,

    /// Path to the payer's keypair file.
    #[structopt(long)]
    payer_filepath: String,

    #[structopt(subcommand)]
    cmd: Command,
}

#[derive(Debug, StructOpt)]
enum Command {
    /// Creates *and* initializes a SRM safe with an admin key.
    CreateAndInitialize {
        /// The key to give admin access to.
        admin_account_owner: Pubkey,
    },
    /// Initializes an SRM safe account with an admin key.
    /// This should only be used for testing.
    Initialize {
        /// The pubkey of the safe account to use.
        safe_account: Pubkey,
        /// The key to give admin access to.
        admin_account_owner: Pubkey,
    },
    /// Slash a vesting account.
    Slash {
        /// Path to the admin's keypair file.
        admin_filepath: String,
        /// The address of the vesting account to slash,
        vesting_account: Pubkey,
    },
    /// Depost SRM into the safe.
    Deposit {
        /// Owner of the vesting account to create.
        vesting_account_owner: String,
    },
    /// Withdraw SRM from a vesting account.
    Withdraw {},
    /// Mint lSRM from a vesting account.
    MintLockedSrm {},
    /// Burn lSRM taken from a vesting account.
    BurnLockedSrm {},
}

fn main() {
    let opt = Opt::from_args();

    let client = Client::from_keypair_file(
        opt.program_id,
        opt.payer_filepath.as_str(),
        opt.cluster.url(),
    )
    .expect("invalid keypair file")
    .with_options(RequestOptions {
        commitment: CommitmentConfig::single(),
        tx: RpcSendTransactionConfig {
            skip_preflight: true,
            preflight_commitment: None,
        },
    });

    let result = match opt.cmd {
        Command::CreateAndInitialize {
            admin_account_owner,
        } => client.mint_locked_srm(&[], 1), //client.create_and_initialize(admin_account_owner),
        Command::Initialize {
            safe_account,
            admin_account_owner,
        } => client.mint_locked_srm(&[], 1), // client.initialize(safe_account, admin_account_owner),
        Command::Slash {
            admin_filepath,
            vesting_account,
        } => client.mint_locked_srm(&[], 1), //slash(admin_filepath, vesting_account),
        Command::Deposit {
            vesting_account_owner,
        } => client.mint_locked_srm(&[], 1), //client.deposit(&client),
        Command::Withdraw {} => client.mint_locked_srm(&[], 1),
        Command::MintLockedSrm {} => client.mint_locked_srm(&[], 1),
        Command::BurnLockedSrm {} => client.mint_locked_srm(&[], 1),
    };

    match result {
        Err(e) => println!("Transaction error: {:?}", e),
        Ok(tx_signature) => println!("Transaction signature: {:?}", tx_signature),
    };
}
