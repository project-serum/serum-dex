use anyhow::{anyhow, Result};
use clap::Clap;
use serum_common::client::rpc;
use serum_context::Context;
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
        /// SRM equivalent amount required for node activation.
        #[clap(short, long, default_value = "10_000_000")]
        reward_activation_threshold: u64,
        #[clap(short, long)]
        max_stake_per_entity: u64,
        #[clap(short, long)]
        stake_rate: u64,
        #[clap(short = 'b', long)]
        stake_rate_mega: u64,
    },
    /// Creates and registers a delegated staked node entity.
    CreateEntity {
        #[clap(short, long)]
        meta_entity_program_id: Pubkey,
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
    /// Sends all leftover funds from an expired unlocked reward vendor to a given
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
    /// Sends all leftover funds from an expired locked reward vendor to a given
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
        /// Address of the stake account [optional]. If not provided, the
        /// first derived stake address will be used for the configured wallet.
        #[clap(short, long)]
        address: Option<Pubkey>,
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
        Command::Accounts(cmd) => account_cmd(&ctx, registry_pid, cmd),
        Command::Init {
            withdrawal_timelock,
            deactivation_timelock,
            reward_activation_threshold,
            max_stake_per_entity,
            stake_rate,
            stake_rate_mega,
        } => init(
            &ctx,
            registry_pid,
            withdrawal_timelock,
            deactivation_timelock,
            reward_activation_threshold,
            max_stake_per_entity,
            stake_rate,
            stake_rate_mega,
        ),
        Command::CreateEntity {
            registrar,
            name,
            about,
            image_url,
            meta_entity_program_id,
        } => create_entity_cmd(
            &ctx,
            registry_pid,
            registrar,
            name,
            about,
            image_url,
            meta_entity_program_id,
        ),
        Command::CreateMember {
            entity,
            delegate,
            registrar,
        } => create_member_cmd(&ctx, registry_pid, registrar, entity, delegate),
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

fn create_member_cmd(
    ctx: &Context,
    registry_pid: Pubkey,
    registrar: Pubkey,
    entity: Pubkey,
    delegate: Option<Pubkey>,
) -> Result<()> {
    let delegate = delegate.unwrap_or(Pubkey::new_from_array([0; 32]));

    let client = ctx.connect::<Client>(registry_pid)?;

    let CreateMemberResponse { tx, member } = client.create_member(CreateMemberRequest {
        entity,
        beneficiary: &ctx.wallet()?,
        delegate,
        registrar,
    })?;

    println!("Confirmed transaction: {:?}", tx);
    println!("Created node entity member with address: {:?}", member);

    Ok(())
}

fn create_entity_cmd(
    ctx: &Context,
    registry_pid: Pubkey,
    registrar: Pubkey,
    name: String,
    about: String,
    image_url: String,
    meta_entity_program_id: Pubkey,
) -> Result<()> {
    let leader_kp = ctx.wallet()?;

    let client = ctx.connect::<Client>(registry_pid)?;
    let CreateEntityResponse { entity, .. } = client.create_entity(CreateEntityRequest {
        node_leader: &leader_kp,
        registrar,
        metadata: Some(EntityMetadata {
            name,
            about,
            image_url,
            meta_entity_program_id,
        }),
    })?;

    println!("{}", serde_json::json!({"entity": entity.to_string()}));

    Ok(())
}

fn account_cmd(ctx: &Context, registry_pid: Pubkey, cmd: AccountsCommand) -> Result<()> {
    let rpc_client = ctx.rpc_client();

    match cmd {
        AccountsCommand::Registrar { address } => {
            let registrar: Registrar = rpc::get_account(&rpc_client, &address)?;
            println!("{:#?}", registrar);
        }
        AccountsCommand::Entity { address } => {
            let acc: Entity = rpc::get_account_unchecked(&rpc_client, &address)?;
            println!("{:#?}", acc);
        }
        AccountsCommand::Member { address } => {
            let address = match address {
                Some(a) => a,
                None => Pubkey::create_with_seed(
                    &ctx.wallet()?.pubkey(),
                    Client::member_seed(),
                    &registry_pid,
                )
                .map_err(|e| anyhow!("unable to derive stake address: {}", e.to_string()))?,
            };
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
    reward_activation_threshold: u64,
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
        reward_activation_threshold,
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
