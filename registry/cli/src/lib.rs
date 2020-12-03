use anyhow::{anyhow, Result};
use clap::Clap;
use serum_common::client::rpc;
use serum_node_context::Context;
use serum_node_logging::info;
use serum_registry::accounts::{Entity, Member, Registrar};
use serum_registry_client::*;
use solana_client_gen::prelude::*;

#[derive(Debug, Clap)]
#[clap(name = "Serum Registry CLI")]
pub struct Opts {
    #[clap(flatten)]
    pub ctx: Context,

    #[clap(flatten)]
    pub cmd: Command,
}

#[derive(Debug, Clap)]
pub struct Command {
    /// Program id of the deployed on-chain registrar
    #[clap(long = "pid")]
    pub registry_pid: Option<Pubkey>,

    #[clap(flatten)]
    pub sub_cmd: SubCommand,
}

#[derive(Debug, Clap)]
pub enum SubCommand {
    /// Commands to view registry owned accounts.
    Accounts(AccountsCommand),
    /// Initializes a registrar.
    Init {
        /// The amount of seconds one must wait for a staking withdrawal.
        #[clap(short, long, default_value = "10000")]
        withdrawal_timelock: i64,
        /// Seoncds until  deactivation.
        #[clap(short = 't', long, default_value = "10000")]
        deactivation_timelock: i64,
        /// SRM equivalent amount required for node activation.
        #[clap(short, long, default_value = "10_000_000")]
        reward_activation_threshold: u64,
        #[clap(short, long)]
        max_stake_per_entity: u64,
    },
    /// Creates and registers a delegated staked node entity.
    CreateEntity {
        #[clap(short, long)]
        meta_entity_program_id: Pubkey,
        /// The keypair filepath for the node leader.
        #[clap(short, long)]
        leader: String,
        /// Registrar account address.
        #[clap(short, long)]
        registrar: Pubkey,
        #[clap(short, long)]
        name: String,
        #[clap(short, long)]
        about: String,
        #[clap(short, long)]
        image_url: String,
    },
    /// Joins an entity, creating an associated member account.
    CreateMember {
        /// Node entity to join with.
        #[clap(short, long)]
        entity: Pubkey,
        /// Delegate of the member account [optional].
        #[clap(short, long)]
        delegate: Option<Pubkey>,
        /// Registrar account address.
        #[clap(short, long)]
        registrar: Pubkey,
    },
}

// AccountsComand defines the subcommand to view formatted account data
// belonging to the registry program.
#[derive(Debug, Clap)]
pub enum AccountsCommand {
    /// View the registrar instance.
    Registrar {
        /// Address of the Registrar instance.
        #[clap(short, long)]
        address: Pubkey,
    },
    /// View a node entity.
    Entity {
        /// Address of the entity account [optional].
        #[clap(short, long, required_unless_present("leader"))]
        address: Option<Pubkey>,
        /// Address of the leader of the entity [optional].
        #[clap(short, long, required_unless_present("address"))]
        leader: Option<Pubkey>,
    },
    /// View a member of a node entity.
    Member {
        /// Address of the stake account [optional]. If not provided, the
        /// first derived stake address will be used for the configured wallet.
        #[clap(short, long)]
        address: Option<Pubkey>,
    },
}

pub fn run(opts: Opts) -> Result<()> {
    let ctx = &opts.ctx;
    let registry_pid = opts.cmd.registry_pid;

    match opts.cmd.sub_cmd {
        SubCommand::Accounts(cmd) => account_cmd(ctx, registry_pid, cmd),
        SubCommand::Init {
            withdrawal_timelock,
            deactivation_timelock,
            reward_activation_threshold,
            max_stake_per_entity,
        } => init(
            ctx,
            registry_pid,
            withdrawal_timelock,
            deactivation_timelock,
            reward_activation_threshold,
            max_stake_per_entity,
        ),
        SubCommand::CreateEntity {
            leader,
            registrar,
            name,
            about,
            image_url,
            meta_entity_program_id,
        } => create_entity_cmd(
            ctx,
            registry_pid,
            registrar,
            leader,
            name,
            about,
            image_url,
            meta_entity_program_id,
        ),
        SubCommand::CreateMember {
            entity,
            delegate,
            registrar,
        } => create_member_cmd(ctx, registry_pid, registrar, entity, delegate),
    }
}

fn create_member_cmd(
    ctx: &Context,
    registry_pid: Option<Pubkey>,
    registrar: Pubkey,
    entity: Pubkey,
    delegate: Option<Pubkey>,
) -> Result<()> {
    let registry_pid = registry_pid.ok_or(anyhow!("--pid not provided"))?;
    let delegate = delegate.unwrap_or(Pubkey::new_from_array([0; 32]));

    let client = ctx.connect::<Client>(registry_pid)?;

    let CreateMemberResponse { tx, member } = client.create_member(CreateMemberRequest {
        entity,
        beneficiary: &ctx.wallet()?,
        delegate,
        registrar,
    })?;

    let logger = serum_node_logging::get_logger("node/registry");
    info!(logger, "Confirmed transaction: {:?}", tx);
    info!(
        logger,
        "Created node entity member with address: {:?}", member
    );

    Ok(())
}

fn create_entity_cmd(
    ctx: &Context,
    registry_pid: Option<Pubkey>,
    registrar: Pubkey,
    leader_filepath: String,
    name: String,
    about: String,
    image_url: String,
    meta_entity_program_id: Pubkey,
) -> Result<()> {
    let registry_pid = registry_pid.ok_or(anyhow!("--pid not provided"))?;

    let leader_kp = solana_sdk::signature::read_keypair_file(&leader_filepath)
        .map_err(|_| anyhow!("Unable to read leader keypair file"))?;

    let client = ctx.connect::<Client>(registry_pid)?;
    let CreateEntityResponse { tx, entity } = client.create_entity(CreateEntityRequest {
        node_leader: &leader_kp,
        registrar,
        name,
        about,
        image_url,
        meta_entity_program_id,
    })?;

    println!("{}", serde_json::json!({"entity": entity.to_string()}));

    Ok(())
}

fn account_cmd(ctx: &Context, registry_pid: Option<Pubkey>, cmd: AccountsCommand) -> Result<()> {
    let rpc_client = ctx.rpc_client();

    match cmd {
        AccountsCommand::Registrar { address } => {
            let registrar: Registrar = rpc::get_account(&rpc_client, &address)?;
            println!("{:#?}", registrar);
        }
        AccountsCommand::Entity { address, leader } => {
            let entity_addr = {
                if let Some(address) = address {
                    address
                } else {
                    let registry_pid = registry_pid.ok_or(anyhow!(
                        "Please provide --pid when looking up entities by node leader"
                    ))?;
                    let leader = leader.expect("address or leader must be present");
                    let seed = "srm:registry:entity";
                    Pubkey::create_with_seed(&leader, &seed, &registry_pid)?
                }
            };

            let acc: Entity = rpc::get_account_unchecked(&rpc_client, &entity_addr)?;
            println!("Address: {}", entity_addr);
            println!("{:#?}", acc);
        }
        AccountsCommand::Member { address } => {
            let address = match address {
                Some(a) => a,
                None => {
                    let registry_pid = registry_pid.ok_or(anyhow!("--pid not provided"))?;
                    Pubkey::create_with_seed(
                        &ctx.wallet()?.pubkey(),
                        Client::member_seed(),
                        &registry_pid,
                    )
                    .map_err(|e| anyhow!("unable to derive stake address: {}", e.to_string()))?
                }
            };
            let acc: Member = rpc::get_account(&rpc_client, &address)?;
            println!("{:#?}", acc);
        }
    };
    Ok(())
}

pub fn init(
    ctx: &Context,
    registry_pid: Option<Pubkey>,
    withdrawal_timelock: i64,
    deactivation_timelock: i64,
    reward_activation_threshold: u64,
    max_stake_per_entity: u64,
) -> Result<()> {
    let registry_pid = registry_pid.ok_or(anyhow!(
        "Please provide --pid when initializing a registrar"
    ))?;

    let client = ctx.connect::<Client>(registry_pid)?;

    let registrar_authority = ctx.wallet()?.pubkey();
    let InitializeResponse {
        registrar,
        reward_event_q,
        nonce,
        ..
    } = client.initialize(InitializeRequest {
        registrar_authority,
        withdrawal_timelock,
        deactivation_timelock,
        mint: ctx.srm_mint,
        mega_mint: ctx.msrm_mint,
        reward_activation_threshold,
        max_stake_per_entity,
    })?;

    println!(
        "{}",
        serde_json::json!({
            "registrar": registrar.to_string(),
            "rewardEventQueue": reward_event_q.to_string(),
            "nonce": nonce.to_string(),
        })
        .to_string()
    );

    Ok(())
}
