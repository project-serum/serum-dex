use rand::rngs::OsRng;
use serum_common::Cluster;
use serum_safe_interface::accounts::{SafeAccount, VestingAccount};
use serum_safe_interface::client::{Client, ClientError, RequestOptions};
use solana_client::rpc_config::RpcSendTransactionConfig;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::instruction::AccountMeta;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, Signature, Signer};
use solana_sdk::transaction::Transaction;
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
    /// Creates *and* initializes a SRM safe with an admin key,
    Initialize {
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
    Deposit {},
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
        Command::Initialize {
            admin_account_owner,
        } => initialize(client, opt.program_id, admin_account_owner),
        Command::Slash {
            admin_filepath,
            vesting_account,
        } => slash(client, admin_filepath, vesting_account),
        Command::Deposit {} => deposit(client),
        Command::Withdraw {} => client.mint_locked_srm(&[], 1),
        Command::MintLockedSrm {} => client.mint_locked_srm(&[], 1),
        Command::BurnLockedSrm {} => client.mint_locked_srm(&[], 1),
    };

    match result {
        Err(e) => println!("Transaction error: {:?}", e),
        Ok(tx_signature) => println!("Transaction signature: {:?}", tx_signature),
    };
}

fn initialize(
    client: Client,
    program_id: Pubkey,
    admin_account_owner: Pubkey,
) -> Result<Signature, ClientError> {
    // The new SRM safe instance.
    let safe_account = Keypair::generate(&mut OsRng);

    let signers = vec![client.payer(), &safe_account];

    let lamports = client
        .rpc()
        .get_minimum_balance_for_rent_exemption(SafeAccount::SIZE)
        .map_err(|e| ClientError::RpcError(e))?;

    let create_account_instr = solana_sdk::system_instruction::create_account(
        &client.payer().pubkey(),
        &safe_account.pubkey(),
        lamports,
        SafeAccount::SIZE as u64,
        &program_id,
    );

    let accounts = &[
        AccountMeta::new(safe_account.pubkey(), false),
        AccountMeta::new_readonly(solana_sdk::sysvar::rent::id(), false),
    ];

    let init_instr =
        serum_safe_interface::instruction::initialize(program_id, accounts, admin_account_owner);

    let instructions = [create_account_instr, init_instr];

    let (recent_hash, _fee_calc) = client
        .rpc()
        .get_recent_blockhash()
        .map_err(|e| ClientError::RawError(e.to_string()))?;

    let tx = Transaction::new_signed_with_payer(
        &instructions,
        Some(&client.payer().pubkey()),
        &signers,
        recent_hash,
    );

    serum_common::rpc::send_txn(client.rpc(), &tx, false)
        .map_err(|e| ClientError::RawError(e.to_string()))
}

fn slash(
    client: Client,
    admin_filepath: String,
    vesting_account: Pubkey,
) -> Result<Signature, ClientError> {
    // TODO: check if the signer is automatically added to the account list
    //       or if we need to do that sepparately.
    let admin_kp = solana_sdk::signature::read_keypair_file(&admin_filepath)
        .expect("admin keypair must be available");
    let accounts = vec![AccountMeta {
        pubkey: vesting_account,
        is_signer: false,
        is_writable: true,
    }];
    let signers = &[&admin_kp];
    client.slash_with_signers(signers, &accounts, 1)
}

fn deposit(client: Client) -> Result<Signature, ClientError> {
    client.mint_locked_srm(&[], 1)
}

fn withdraw(client: Client) -> Result<Signature, ClientError> {
    client.mint_locked_srm(&[], 1)
}

fn mint_locked_srm(client: Client) -> Result<Signature, ClientError> {
    client.mint_locked_srm(&[], 1)
}

fn burn_locked_srm(client: Client) -> Result<Signature, ClientError> {
    client.mint_locked_srm(&[], 1)
}
