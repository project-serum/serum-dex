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
    #[clap(flatten)]
    pub sub_cmd: SubCommand,
}

#[derive(Debug, Clap)]
pub enum SubCommand {
    /// Commands to view program owned accounts.
    Accounts(AccountsCommand),
    /// Governance commands requiring an authority key.
    Gov {
        #[clap(short = 'f', long)]
        authority_file: String,
        #[clap(flatten)]
        cmd: GovCommand,
    },
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
    /// Initializes a safe.
    Initialize,
    /// Adds a program to the whitelist.
    WhitelistAdd {
        /// Safe to update.
        #[clap(short, long)]
        safe: Pubkey,
        /// Program to add to the whitelist.
        #[clap(short, long)]
        program_id: Pubkey,
    },
    /// Removes a program from the whitelist.
    WhitelistDelete {
        /// Safe to update.
        #[clap(short, long)]
        safe: Pubkey,
        /// Program to delete from the whitelist.
        #[clap(short, long)]
        program_id: Pubkey,
    },
    /// Sets a new authority on the safe instance.
    SetAuthority {
        /// Safe to update.
        #[clap(short, long)]
        safe: Pubkey,
        /// Pubkey of the new safe authority.
        #[clap(short, long)]
        new_authority: Pubkey,
    },
    /// Migrates the safe sending all the funds to a new account.
    Migrate {
        /// Safe to migrate.
        #[clap(short, long)]
        safe: Pubkey,
        /// Token account to send the safe to.
        #[clap(short, long)]
        new_token_account: Pubkey,
    },
}

pub fn run(opts: Opts) -> Result<()> {
    let ctx = &opts.ctx;

    match opts.cmd.sub_cmd {
        SubCommand::Accounts(cmd) => account_cmd(ctx, cmd),
        SubCommand::Gov {
            authority_file,
            cmd,
        } => gov_cmd(ctx, authority_file, cmd),
    }
}

fn account_cmd(ctx: &Context, cmd: AccountsCommand) -> Result<()> {
    match cmd {
        AccountsCommand::Safe { address } => {
            // todo
            Ok(())
        }
        AccountsCommand::Vesting {
            address,
            beneficiary,
        } => {
            // todo
            Ok(())
        }
        AccountsCommand::Whitelist { safe } => {
            // todo
            Ok(())
        }
        AccountsCommand::Vault { safe } => {
            // todo
            Ok(())
        }
    }
}

fn gov_cmd(ctx: &Context, authority_file: String, cmd: GovCommand) -> Result<()> {
    match cmd {
        GovCommand::Initialize => {
            // todo
            Ok(())
        }
        GovCommand::WhitelistAdd { safe, program_id } => {
            // todo
            Ok(())
        }
        GovCommand::WhitelistDelete { safe, program_id } => {
            // todo
            Ok(())
        }
        GovCommand::SetAuthority {
            safe,
            new_authority,
        } => {
            // todo
            Ok(())
        }
        GovCommand::Migrate {
            safe,
            new_token_account,
        } => {
            // todo
            Ok(())
        }
        _ => {
            // todo
            Ok(())
        }
    }
}
