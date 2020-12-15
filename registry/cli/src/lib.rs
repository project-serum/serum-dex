use anyhow::Result;
use clap::Clap;
use serum_common::client::rpc;
use serum_context::Context;
use serum_meta_entity::accounts::Metadata;
use serum_registry::accounts::{
    Entity, LockedRewardVendor, Member, Registrar, UnlockedRewardVendor,
};
use serum_registry_client::*;
use solana_client_gen::prelude::*;

#[derive(Debug, Clap)]
pub enum Command {
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
        #[clap(short, long)]
        max_stake_per_entity: u64,
        #[clap(short, long)]
        stake_rate: u64,
        #[clap(short = 'b', long)]
        stake_rate_mega: u64,
    },
    /// Creates a node entity, setting the active wallet as leader.
    CreateEntity {
        /// Registrar account address.
        #[clap(short, long)]
        registrar: Pubkey,
        #[clap(short, long)]
        name: String,
        #[clap(short, long)]
        about: String,
        #[clap(short, long)]
        image_url: Option<String>,
    },
    /// Updates an entity. Active wallet must be the node leader.
    UpdateEntity {
        #[clap(short, long)]
        name: Option<String>,
        #[clap(short, long)]
        about: Option<String>,
        #[clap(short, long)]
        image_url: Option<String>,
        #[clap(short, long)]
        entity: Pubkey,
    },
    /// Sends all unclaimed funds from an expired unlocked reward vendor to a given
    /// account.
    ExpireUnlockedReward {
        /// The token account to send the leftover rewards to.
        #[clap(long)]
        token: Pubkey,
        #[clap(long)]
        vendor: Pubkey,
        #[clap(short, long)]
        registrar: Pubkey,
    },
    /// Sends all unclaimed funds from an expired locked reward vendor to a given
    /// account.
    ExpireLockedReward {
        /// The token account to send the leftover rewards to.
        #[clap(long)]
        token: Pubkey,
        #[clap(long)]
        vendor: Pubkey,
        #[clap(short, long)]
        registrar: Pubkey,
    },
}

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
        /// Address of the entity account.
        #[clap(short, long)]
        address: Pubkey,
    },
    /// View a member of a node entity.
    Member {
        /// Address of the member stake account.
        #[clap(short, long)]
        address: Pubkey,
    },
    LockedVendor {
        #[clap(short, long)]
        address: Pubkey,
    },
    UnlockedVendor {
        #[clap(short, long)]
        address: Pubkey,
    },
}

pub fn run(ctx: Context, cmd: Command) -> Result<()> {
    let registry_pid = ctx.registry_pid;

    match cmd {
        Command::Accounts(cmd) => account_cmd(&ctx, cmd),
        Command::Init {
            withdrawal_timelock,
            deactivation_timelock,
            max_stake_per_entity,
            stake_rate,
            stake_rate_mega,
        } => init(
            &ctx,
            registry_pid,
            withdrawal_timelock,
            deactivation_timelock,
            max_stake_per_entity,
            stake_rate,
            stake_rate_mega,
        ),
        Command::CreateEntity {
            registrar,
            name,
            about,
            image_url,
        } => create_entity_cmd(&ctx, registry_pid, registrar, name, about, image_url),
        Command::UpdateEntity {
            name,
            about,
            image_url,
            entity,
        } => update_entity_cmd(&ctx, name, about, image_url, entity),
        Command::ExpireUnlockedReward {
            token,
            vendor,
            registrar,
        } => {
            let client = ctx.connect::<Client>(registry_pid)?;
            let resp = client.expire_unlocked_reward(ExpireUnlockedRewardRequest {
                token,
                vendor,
                registrar,
            })?;
            println!("Transaction executed: {:?}", resp.tx);
            Ok(())
        }
        Command::ExpireLockedReward {
            token,
            vendor,
            registrar,
        } => {
            let client = ctx.connect::<Client>(registry_pid)?;
            let resp = client.expire_locked_reward(ExpireLockedRewardRequest {
                token,
                vendor,
                registrar,
            })?;
            println!("Transaction executed: {:?}", resp.tx);
            Ok(())
        }
    }
}

fn create_entity_cmd(
    ctx: &Context,
    registry_pid: Pubkey,
    registrar: Pubkey,
    name: String,
    about: String,
    image_url: Option<String>,
) -> Result<()> {
    let leader_kp = ctx.wallet()?;

    let client = ctx.connect::<Client>(registry_pid)?;
    let CreateEntityResponse { entity, .. } = client.create_entity(CreateEntityRequest {
        node_leader: &leader_kp,
        registrar,
        metadata: Some(EntityMetadata {
            name,
            about,
            image_url: image_url.unwrap_or("".to_string()),
            meta_entity_program_id: ctx.meta_entity_pid,
        }),
    })?;

    println!("{}", serde_json::json!({"entity": entity.to_string()}));

    Ok(())
}

fn update_entity_cmd(
    ctx: &Context,
    name: Option<String>,
    about: Option<String>,
    image_url: Option<String>,
    entity: Pubkey,
) -> Result<()> {
    let client = ctx.connect::<Client>(ctx.registry_pid)?;
    let resp = client.update_entity_metadata(UpdateEntityMetadataRequest {
        name,
        about,
        image_url,
        entity,
        meta_entity_pid: ctx.meta_entity_pid,
    })?;
    println!("Transaction signature: {}", resp.tx.to_string());

    Ok(())
}

fn account_cmd(ctx: &Context, cmd: AccountsCommand) -> Result<()> {
    let rpc_client = ctx.rpc_client();

    match cmd {
        AccountsCommand::Registrar { address } => {
            let registrar: Registrar = rpc::get_account(&rpc_client, &address)?;
            println!("{:#?}", registrar);
        }
        AccountsCommand::Entity { address } => {
            let acc: Entity = rpc::get_account_unchecked(&rpc_client, &address)?;
            let m: Metadata = rpc::get_account_unchecked(&rpc_client, &acc.metadata)?;
            println!("{:#?}", acc);
            println!("{:#?}", m);
        }
        AccountsCommand::Member { address } => {
            let acc: Member = rpc::get_account(&rpc_client, &address)?;
            println!("{:#?}", acc);
        }
        AccountsCommand::LockedVendor { address } => {
            let acc: LockedRewardVendor = rpc::get_account(&rpc_client, &address)?;
            println!("{:#?}", acc);
        }
        AccountsCommand::UnlockedVendor { address } => {
            let acc: UnlockedRewardVendor = rpc::get_account(&rpc_client, &address)?;
            println!("{:#?}", acc);
        }
    };
    Ok(())
}

pub fn init(
    ctx: &Context,
    registry_pid: Pubkey,
    withdrawal_timelock: i64,
    deactivation_timelock: i64,
    max_stake_per_entity: u64,
    stake_rate: u64,
    stake_rate_mega: u64,
) -> Result<()> {
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
        max_stake_per_entity,
        stake_rate,
        stake_rate_mega,
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
