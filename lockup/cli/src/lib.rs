use anyhow::{anyhow, Result};
use clap::Clap;
use serum_lockup::accounts::WhitelistEntry;
use serum_lockup_client::*;
use serum_node_context::Context;
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
    /// Program id of the lockup program.
    #[clap(short, long = "pid")]
    pub pid: Pubkey,
    #[clap(flatten)]
    pub sub_cmd: SubCommand,
}

#[derive(Debug, Clap)]
pub enum SubCommand {
    /// Commands to view program owned accounts.
    Accounts(AccountsCommand),
    /// Governance commands requiring an authority key.
    Gov {
        /// Filepath to the authority key.
        #[clap(short = 'f', long)]
        authority_file: String,
        /// Safe account to govern.
        #[clap(short, long)]
        safe: Pubkey,
        #[clap(flatten)]
        cmd: GovCommand,
    },
    /// Initializes a Safe.
    Initialize {
        /// Authority to set on the new safe.
        #[clap(short, long)]
        authority: Pubkey,
    },
    /// Creates a vesting account.
    CreateVesting {
        /// Token account sending funds.
        #[clap(short, long)]
        depositor: Pubkey,
        /// Safe to associate this Vesting account with.
        #[clap(short, long)]
        safe: Pubkey,
        /// Beneficiary address to give this Vesting account to.
        #[clap(short, long)]
        beneficiary: Pubkey,
        /// Slot at which point the entire account is vested.
        #[clap(short, long)]
        end_ts: i64,
        /// Number of vesting periods for this account.
        #[clap(short, long)]
        period_count: u64,
        /// Amount of tokens to give this Vesting account.
        #[clap(short = 'a', long)]
        deposit_amount: u64,
    },
    /// Claim a vesting account, receiving a non-fungible token receipt.
    Claim {
        /// The vesting account to claim.
        #[clap(short, long)]
        vesting: Pubkey,
    },
    /// Redeem a claimed token receipt for an amount of vested tokens.
    Redeem {
        /// The amount of vested tokens to redeem.
        #[clap(short, long)]
        amount: u64,
        /// Vesting account to redeem from.
        #[clap(short, long)]
        vesting: Pubkey,
        /// Token account to send the vested tokens to.
        #[clap(short, long)]
        token_account: Pubkey,
    },
}

#[derive(Debug, Clap)]
pub enum AccountsCommand {
    /// View the Safe account.
    Safe {
        /// Address of the safe instance.
        #[clap(short, long)]
        address: Pubkey,
    },
    /// View a Vesting account.
    Vesting {
        /// Address of the vesting account.
        #[clap(short, long)]
        address: Pubkey,
    },
    /// View the Safe's whitelist.
    Whitelist {
        /// Address of the safe instance.
        #[clap(short, long)]
        safe: Pubkey,
    },
    /// View the Safe's token vault.
    Vault {
        /// Address of the safe instance.
        #[clap(short, long)]
        safe: Pubkey,
    },
}

/// Governance commands requiring an authority key.
#[derive(Debug, Clap)]
pub enum GovCommand {
    /// Adds a program to the whitelist.
    WhitelistAdd {
        /// WhitelistEntry program id.
        #[clap(short, long)]
        program_id: Pubkey,
        /// WhitelistEntry signer-seeds instance.
        #[clap(short, long)]
        instance: Pubkey,
        /// WhitelistEntry signer-seeds nonce.
        #[clap(short, long)]
        nonce: u8,
    },
    /// Removes a program from the whitelist.
    WhitelistDelete {
        /// WhitelistEntry program id.
        #[clap(short, long)]
        program_id: Pubkey,
        /// WhitelistEntry signer-seeds instance.
        #[clap(short, long)]
        instance: Pubkey,
        /// WhitelistEntry signer-seeds nonce.
        #[clap(short, long)]
        nonce: u8,
    },
    /// Sets a new authority on the safe instance.
    SetAuthority {
        /// Pubkey of the new safe authority.
        #[clap(short, long)]
        new_authority: Pubkey,
    },
    /// Migrates the safe sending all the funds to a new account.
    Migrate {
        /// Token account to send the safe to.
        #[clap(short, long)]
        new_token_account: Pubkey,
    },
}

pub fn run(opts: Opts) -> Result<()> {
    let ctx = &opts.ctx;

    match opts.cmd.sub_cmd {
        SubCommand::Accounts(cmd) => account_cmd(ctx, opts.cmd.pid, cmd),
        SubCommand::Gov {
            authority_file,
            safe,
            cmd,
        } => gov_cmd(ctx, opts.cmd.pid, authority_file, safe, cmd),
        SubCommand::Initialize { authority } => {
            let client = ctx.connect::<Client>(opts.cmd.pid)?;
            let resp = client.initialize(InitializeRequest {
                mint: ctx.srm_mint,
                authority: authority,
            })?;
            println!("{:#?}", resp);
            Ok(())
        }
        SubCommand::CreateVesting {
            depositor,
            safe,
            beneficiary,
            end_ts,
            period_count,
            deposit_amount,
        } => {
            let client = ctx.connect::<Client>(opts.cmd.pid)?;
            let resp = client.create_vesting(CreateVestingRequest {
                depositor,
                depositor_owner: &ctx.wallet()?,
                safe,
                beneficiary,
                end_ts,
                period_count,
                deposit_amount,
            })?;
            println!("{:#?}", resp);
            Ok(())
        }
        SubCommand::Claim { vesting } => {
            let client = ctx.connect::<Client>(opts.cmd.pid)?;
            let beneficiary = ctx.wallet()?;
            let v_acc = client.vesting(&vesting)?;
            let safe = v_acc.safe;
            let resp = client.claim(ClaimRequest {
                beneficiary: &beneficiary,
                safe,
                vesting,
            })?;
            println!("{:#?}", resp);
            Ok(())
        }
        SubCommand::Redeem {
            vesting,
            amount,
            token_account,
        } => {
            let beneficiary = ctx.wallet()?;
            let client = ctx.connect::<Client>(opts.cmd.pid)?;
            let vesting_account = client.vesting(&vesting)?;
            let safe = vesting_account.safe;
            let resp = client.redeem(RedeemRequest {
                beneficiary: &beneficiary,
                vesting,
                token_account,
                safe,
                amount,
            })?;
            println!("{:#?}", resp);
            Ok(())
        }
    }
}

fn account_cmd(ctx: &Context, pid: Pubkey, cmd: AccountsCommand) -> Result<()> {
    let client = Client::new(ctx.connect(pid)?);
    match cmd {
        AccountsCommand::Safe { address } => {
            let safe = client.safe(&address)?;
            println!("{:#?}", safe);
            Ok(())
        }
        AccountsCommand::Vesting { address } => {
            let vault = client.vesting(&address)?;
            println!("{:#?}", vault);

            let current_ts = client.rpc().get_block_time(client.rpc().get_slot()?)?;
            let amount = vault.available_for_withdrawal(current_ts);
            println!("Redeemable balance: {:?}", amount);
            println!("Whitelistable balance: {:?}", amount);

            Ok(())
        }
        AccountsCommand::Whitelist { safe } => {
            client.with_whitelist(&safe, |whitelist| {
                println!("{:#?}", whitelist);
            })?;
            Ok(())
        }
        AccountsCommand::Vault { safe } => {
            let vault = client.vault(&safe)?;
            println!("{:#?}", vault);
            Ok(())
        }
    }
}

fn gov_cmd(
    ctx: &Context,
    pid: Pubkey,
    authority_file: String,
    safe: Pubkey,
    cmd: GovCommand,
) -> Result<()> {
    let client = ctx.connect::<Client>(pid)?;
    let authority = solana_sdk::signature::read_keypair_file(&authority_file)
        .map_err(|_| anyhow!("Unable to read leader keypair file"))?;
    match cmd {
        GovCommand::WhitelistAdd {
            program_id,
            instance,
            nonce,
        } => {
            client.whitelist_add(WhitelistAddRequest {
                authority: &authority,
                safe,
                entry: WhitelistEntry::new(program_id, instance, nonce),
            })?;
        }
        GovCommand::WhitelistDelete {
            program_id,
            instance,
            nonce,
        } => {
            client.whitelist_delete(WhitelistDeleteRequest {
                authority: &authority,
                safe,
                entry: WhitelistEntry::new(program_id, instance, nonce),
            })?;
        }
        GovCommand::SetAuthority { new_authority } => {
            client.set_authority(SetAuthorityRequest {
                authority: &authority,
                safe,
                new_authority,
            })?;
        }
        GovCommand::Migrate { new_token_account } => {
            client.migrate(MigrateRequest {
                authority: &authority,
                safe,
                new_token_account,
            })?;
        }
    }
    Ok(())
}
