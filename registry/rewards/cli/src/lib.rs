use anyhow::{anyhow, Result};
use clap::Clap;
use serum_node_context::Context;
use serum_rewards_client::*;
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
    /// Program id of the deployed on-chain rewards program.
    #[clap(long = "pid")]
    pub rewards_pid: Option<Pubkey>,
    #[clap(flatten)]
    pub sub_cmd: SubCommand,
}

#[derive(Debug, Clap)]
pub enum SubCommand {
    /// Commands to view rewards owned accounts.
    Accounts(AccountsCommand),
    /// Governance commands requiring an authority key.
    Gov(GovCommand),
}

#[derive(Debug, Clap)]
pub enum AccountsCommand {
    /// View the Instance account representing the rewards.
    Instance {
        /// Instance address.
        #[clap(short, long)]
        address: Pubkey,
    },
    /// View the Instance's vault.
    Vault {
        /// Instance address.
        #[clap(short, long)]
        instance: Pubkey,
    },
}

#[derive(Debug, Clap)]
pub enum GovCommand {
    /// Initializes a rewards instance.
    Init {
        #[clap(short = 'p', long)]
        registry_program_id: Pubkey,
        #[clap(short, long)]
        registrar: Pubkey,
        #[clap(short = 'm', long)]
        reward_mint: Pubkey,
        #[clap(short, long)]
        dex_program_id: Pubkey,
    },
    /// Sets a new authority on the instance.
    SetAuthority {
        new_authority: Pubkey,
        instance: Pubkey,
    },
    /// Migrates all funds to the given address. Authority must be configured
    /// wallet.
    Migrate { instance: Pubkey, receiver: Pubkey },
}

pub fn run(opts: Opts) -> Result<()> {
    let ctx = &opts.ctx;
    let rewards_pid = opts.cmd.rewards_pid;

    match opts.cmd.sub_cmd {
        SubCommand::Accounts(cmd) => account_cmd(ctx, rewards_pid, cmd),
        SubCommand::Gov(cmd) => gov_cmd(ctx, rewards_pid, cmd),
    }
}

fn account_cmd(ctx: &Context, rewards_pid: Option<Pubkey>, cmd: AccountsCommand) -> Result<()> {
    let rewards_pid = rewards_pid.ok_or(anyhow!("--pid not provided"))?;
    let client = ctx.connect::<Client>(rewards_pid)?;
    match cmd {
        AccountsCommand::Instance { address } => {
            let instance = client.instance(address)?;
            println!("{:#?}", instance);
        }
        AccountsCommand::Vault { instance } => {
            let vault = client.vault(instance)?;
            println!("{:#?}", vault);
        }
    }

    Ok(())
}

fn gov_cmd(ctx: &Context, rewards_pid: Option<Pubkey>, cmd: GovCommand) -> Result<()> {
    let rewards_pid = rewards_pid.ok_or(anyhow!("--pid not provided"))?;
    let client = ctx.connect::<Client>(rewards_pid)?;
    let wallet = ctx.wallet()?;
    match cmd {
        GovCommand::Init {
            registry_program_id,
            registrar,
            reward_mint,
            dex_program_id,
        } => {
            client.initialize(InitializeRequest {
                registry_program_id,
                registrar,
                reward_mint,
                dex_program_id,
                authority: wallet.pubkey(),
            })?;
        }
        GovCommand::SetAuthority {
            new_authority,
            instance,
        } => {
            client.set_authority(SetAuthorityRequest {
                instance,
                new_authority,
                authority: &wallet,
            })?;
        }
        GovCommand::Migrate { instance, receiver } => {
            client.migrate(MigrateRequest {
                authority: &wallet,
                instance,
                receiver,
            })?;
        }
    }
    Ok(())
}
