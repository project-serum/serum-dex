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
    /// Governance commands requiring an authority key.
    Gov(GovCommand),
    /// Creates and registers a delegated staked node entity.
    CreateEntity {
        /// The keypair filepath for the node leader.
        #[clap(short, long)]
        leader: String,
        /// Flag for specifiying the crank capability. Required.
        #[clap(short, long)]
        crank: bool,
        /// Registrar account address.
        #[clap(short, long)]
        registrar: Pubkey,
    },
    /// Joins an entity, creating an associated member account.
    JoinEntity {
        /// Node entity to join with.
        #[clap(short, long)]
        entity: Pubkey,
        /// Beneficiary of the stake account being created.
        #[clap(short, long)]
        beneficiary: Pubkey,
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

/// Governance commands requiring an authority key.
#[derive(Debug, Clap)]
pub enum GovCommand {
    /// Initializes a registrar.
    Init {
        /// Not required if authority_file is present.
        #[clap(short, long, required_unless_present("authority-file"))]
        authority: Option<Pubkey>,
        /// Not required if authority is present.
        #[clap(short = 'f', long, required_unless_present("authority"))]
        authority_file: Option<String>,
        /// The amount of slots one must wait for a staking withdrawal.
        #[clap(short, long, default_value = "10000")]
        withdrawal_timelock: i64,
        /// Slots in addition to the withdrawal_timelock for deactivation.
        #[clap(short, long, default_value = "10000")]
        deactivation_timelock_premium: i64,
        /// SRM equivalent amount required for node activation.
        #[clap(short, long, default_value = "10_000_000")]
        reward_activation_threshold: u64,
    },
    /// Registers a new node capability in the registrar.
    RegisterCapability {
        /// Force override the capability with this id.
        #[clap(long)]
        force_id: Option<u8>,
        /// Capability fee rate in basis points.
        #[clap(long)]
        fee_bps: u32,
        /// Adress of an initialized on-chain registrar
        #[clap(long)]
        registrar: Pubkey,
        /// Registrar authority key for signing.
        #[clap(long = "authority-file")]
        registrar_authority_file: String,
    },
}

pub fn run(opts: Opts) -> Result<()> {
    let ctx = &opts.ctx;
    let registry_pid = opts.cmd.registry_pid;

    match opts.cmd.sub_cmd {
        SubCommand::Accounts(cmd) => account_cmd(ctx, registry_pid, cmd),
        SubCommand::Gov(cmd) => gov_cmd(ctx, registry_pid, cmd),
        SubCommand::CreateEntity {
            crank,
            leader,
            registrar,
        } => create_entity_cmd(ctx, registry_pid, registrar, leader, crank),
        SubCommand::JoinEntity {
            entity,
            beneficiary,
            delegate,
            registrar,
        } => join_entity_cmd(ctx, registry_pid, registrar, entity, beneficiary, delegate),
    }
}

fn join_entity_cmd(
    ctx: &Context,
    registry_pid: Option<Pubkey>,
    registrar: Pubkey,
    entity: Pubkey,
    beneficiary: Pubkey,
    delegate: Option<Pubkey>,
) -> Result<()> {
    let registry_pid = registry_pid.ok_or(anyhow!("--pid not provided"))?;
    let delegate = delegate.unwrap_or(Pubkey::new_from_array([0; 32]));

    let client = ctx.connect::<Client>(registry_pid)?;

    let watchtower = Pubkey::new_from_array([0; 32]);
    let watchtower_dest = Pubkey::new_from_array([0; 32]);

    let JoinEntityResponse { tx, member } = client.join_entity(JoinEntityRequest {
        entity,
        beneficiary,
        delegate,
        watchtower,
        watchtower_dest,
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
    crank: bool,
) -> Result<()> {
    let registry_pid = registry_pid.ok_or(anyhow!("--pid not provided"))?;
    if !crank {
        return Err(anyhow!("All nodes must crank for this version"));
    }
    // TODO: we should map the set of given capability booleans to this bitmap.
    //       For now we only allow cranking.
    let capabilities = 1;

    let leader_kp = solana_sdk::signature::read_keypair_file(&leader_filepath)
        .map_err(|_| anyhow!("Unable to read leader keypair file"))?;

    let client = ctx.connect::<Client>(registry_pid)?;
    let CreateEntityResponse { tx, entity } = client.create_entity(CreateEntityRequest {
        node_leader: &leader_kp,
        capabilities,
        registrar,
        stake_kind: serum_registry::accounts::StakeKind::Delegated,
    })?;

    let logger = serum_node_logging::get_logger("node/registry");
    info!(logger, "Confirmed transaction: {:?}", tx);
    info!(logger, "Created entity with address: {:?}", entity);

    Ok(())
}

pub fn gov_cmd(ctx: &Context, registry_pid: Option<Pubkey>, gov_cmd: GovCommand) -> Result<()> {
    let registry_pid = registry_pid.ok_or(anyhow!("--pid not provided"))?;
    match gov_cmd {
        GovCommand::Init {
            authority,
            authority_file,
            withdrawal_timelock,
            deactivation_timelock_premium,
            reward_activation_threshold,
        } => gov::init(
            ctx,
            registry_pid,
            authority,
            authority_file,
            withdrawal_timelock,
            deactivation_timelock_premium,
            reward_activation_threshold,
        ),
        GovCommand::RegisterCapability {
            force_id,
            registrar,
            registrar_authority_file,
            fee_bps,
        } => gov::register_capability(
            ctx,
            registry_pid,
            registrar,
            registrar_authority_file,
            force_id,
            fee_bps,
        ),
    }
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

            let acc: Entity = rpc::get_account(&rpc_client, &entity_addr)?;
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

mod gov {
    use super::*;

    pub fn init(
        ctx: &Context,
        registry_pid: Pubkey,
        authority: Option<Pubkey>,
        authority_file: Option<String>,
        withdrawal_timelock: i64,
        deactivation_timelock_premium: i64,
        reward_activation_threshold: u64,
    ) -> Result<()> {
        let logger = serum_node_logging::get_logger("node/registry");

        let client = ctx.connect::<Client>(registry_pid)?;

        let registrar_authority = match authority {
            Some(a) => a,
            None => {
                let file = authority_file.expect("Must be provided if authority is none");
                let kp = solana_sdk::signature::read_keypair_file(&file)
                    .map_err(|_| anyhow!("Unable to read provided authority file"))?;
                kp.pubkey()
            }
        };
        let InitializeResponse {
            tx: _,
            registrar,
            nonce: _,
        } = client.initialize(InitializeRequest {
            registrar_authority,
            withdrawal_timelock,
            deactivation_timelock_premium,
            mint: ctx.srm_mint,
            mega_mint: ctx.msrm_mint,
            reward_activation_threshold,
        })?;

        info!(
            logger,
            "Registrar initialized with address: {:?}", registrar,
        );

        Ok(())
    }

    pub fn register_capability(
        ctx: &Context,
        registry_pid: Pubkey,
        registrar: Pubkey,
        registrar_authority_file: String,
        force_id: Option<u8>,
        capability_fee_bps: u32,
    ) -> Result<()> {
        let logger = serum_node_logging::get_logger("node/registry");
        let client = ctx.connect::<Client>(registry_pid)?;

        let capability_id = match force_id {
            Some(id) => id,
            None => {
                let registrar_acc: Registrar = rpc::get_account(client.rpc(), &registrar)?;
                match registrar_acc.next_free_capability_id() {
                    None => return Err(anyhow!("No available capability slots left")),
                    Some(cap_id) => cap_id,
                }
            }
        };
        let registrar_authority =
            solana_sdk::signature::read_keypair_file(&registrar_authority_file)
                .map_err(|_| anyhow!("Unable to read provided authority file"))?;
        let resp = client.register_capability(RegisterCapabilityRequest {
            registrar,
            registrar_authority: &registrar_authority,
            capability_id,
            capability_fee_bps,
        })?;

        info!(
            logger,
            "Registered capability with transaction signature: {:?}", resp.tx,
        );

        Ok(())
    }
}
