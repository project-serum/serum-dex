use anyhow::{anyhow, Result};
use clap::Clap;
use serum_common::client::rpc;
use serum_node_context::Context;
use serum_safe::accounts::{Safe, Vesting};
use serum_safe::client_ext::client::Client;
use solana_client_gen::prelude::*;

#[derive(Debug, Clap)]
#[clap(name = "Serum Lockup CLI")]
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
    /// Commands to view program owned accounts.
    Accounts(AccountsCommand),
    /// Governance commands requiring an authority key.
    Gov(GovCommand),
}

// AccountsComand defines the subcommand to view formatted account data
// belonging to the registry program.
#[derive(Debug, Clap)]
pub enum AccountsCommand {
    /// View the Safe account.
    Safe {
        /// Address of the safe instance.
        #[clap(short, long)]
        address: Pubkey,
    },
    /// View a vesting account.
    Vesting {
        /// Address of the vesting account [optional].
        #[clap(short, long, required_unless_present("beneficiary"))]
        address: Option<Pubkey>,
        /// Address of the beneficiary of the vesting account [optional].
        #[clap(short, long, required_unless_present("address"))]
        beneficiary: Option<Pubkey>,
    },
    /// View the safe's whitelist.
    Whitelist {
        /// Address of the safe instance.
        #[clap(short, long)]
        safe: Option<Pubkey>,
    },
    /// View the safe's token vault.
    Vault {
        /// Address of the safe instance.
        #[clap(short, long)]
        safe: Pubkey,
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
    },
}

pub fn run(opts: Opts) -> Result<()> {
    let ctx = &opts.ctx;
    let registry_pid = opts.cmd.registry_pid;

    match opts.cmd.sub_cmd {
        SubCommand::Accounts(cmd) => account_cmd(ctx, cmd),
        SubCommand::Gov(cmd) => gov_cmd(ctx, cmd),
    }
}

fn account_cmd(ctx: &Context, cmd: AccountsCommand) -> Result<()> {
    // todo

    Ok(())
}

fn gov_cmd(ctx: &Context, cmd: GovCommand) -> Result<()> {
    // todo

    Ok(())
}
