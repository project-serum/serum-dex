use anyhow::Result;
use clap::Clap;
use serum_context::Context;
use serum_registry_rewards_client::*;
use solana_client_gen::prelude::*;

#[derive(Debug, Clap)]
#[clap(name = "Serum Registry CLI")]
pub struct Opts {
    #[clap(flatten)]
    pub cmd: Command,
}

#[derive(Debug, Clap)]
pub enum Command {
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
        #[clap(short, long)]
        registrar: Pubkey,
        #[clap(short = 'm', long)]
        reward_mint: Pubkey,
        #[clap(short, long)]
        fee_rate: u64,
    },
    /// Sets a new authority on the instance.
    SetAuthority {
        new_authority: Pubkey,
        instance: Pubkey,
    },
    /// Migrates all funds to the given address.
    Migrate { instance: Pubkey, receiver: Pubkey },
}

pub fn run(ctx: Context, cmd: Command) -> Result<()> {
    match cmd {
        Command::Accounts(cmd) => account_cmd(&ctx, cmd),
        Command::Gov(cmd) => gov_cmd(&ctx, cmd),
    }
}

fn account_cmd(ctx: &Context, cmd: AccountsCommand) -> Result<()> {
    let rewards_pid = ctx.rewards_pid;
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

fn gov_cmd(ctx: &Context, cmd: GovCommand) -> Result<()> {
    let rewards_pid = ctx.rewards_pid;
    let client = ctx.connect::<Client>(rewards_pid)?;
    let wallet = ctx.wallet()?;
    match cmd {
        GovCommand::Init {
            registrar,
            reward_mint,
            fee_rate,
        } => {
            let resp = client.initialize(InitializeRequest {
                registry_program_id: ctx.registry_pid,
                registrar,
                reward_mint,
                dex_program_id: ctx.dex_pid,
                fee_rate,
                authority: wallet.pubkey(),
            })?;
            println!("Rewards instance created: {:?}", resp.instance);
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
