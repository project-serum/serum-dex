use anyhow::Result;
use clap::Clap;
use serum_context::Context;
use serum_lockup::accounts::WhitelistEntry;
use serum_lockup_client::*;
use solana_client_gen::prelude::*;

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
        /// Safe account to govern.
        #[clap(short, long)]
        safe: Pubkey,
        #[clap(flatten)]
        cmd: GovCommand,
    },
    /// Initializes a Safe.
    Initialize,
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
    /// Withdraw vested tokens.
    Withdraw {
        /// The amount of vested tokens to withdraw.
        #[clap(short, long)]
        amount: u64,
        /// Vesting account to withdraw from.
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
        /// Address of the vesting account.
        #[clap(short, long)]
        vesting: Pubkey,
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
        instance: Option<Pubkey>,
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
        instance: Option<Pubkey>,
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
}

pub fn run(ctx: Context, cmd: Command) -> Result<()> {
    match cmd.sub_cmd {
        SubCommand::Accounts(accs_cmd) => account_cmd(&ctx, accs_cmd),
        SubCommand::Gov { safe, cmd: gcmd } => gov_cmd(&ctx, safe, gcmd),
        SubCommand::Initialize => {
            let client = ctx.connect::<Client>(ctx.lockup_pid)?;
            let authority = ctx.wallet()?.pubkey();
            let resp = client.initialize(InitializeRequest { authority })?;

            println!("{}", serde_json::json!({"safe": resp.safe.to_string()}));
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
            let client = ctx.connect::<Client>(ctx.lockup_pid)?;
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
        SubCommand::Withdraw {
            vesting,
            amount,
            token_account,
        } => {
            let beneficiary = ctx.wallet()?;
            let client = ctx.connect::<Client>(ctx.lockup_pid)?;
            let vesting_account = client.vesting(&vesting)?;
            let safe = vesting_account.safe;
            let resp = client.withdraw(WithdrawRequest {
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

fn account_cmd(ctx: &Context, cmd: AccountsCommand) -> Result<()> {
    let pid = ctx.lockup_pid;
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
            println!("Withdrawable balance: {:?}", amount);
            println!("Whitelistable balance: {:?}", amount);

            Ok(())
        }
        AccountsCommand::Whitelist { safe } => {
            client.with_whitelist(&safe, |whitelist| {
                println!("{:#?}", whitelist.entries());
            })?;
            Ok(())
        }
        AccountsCommand::Vault { vesting } => {
            let vault = client.vault_for(&vesting)?;
            println!("{:#?}", vault);
            Ok(())
        }
    }
}

fn gov_cmd(ctx: &Context, safe: Pubkey, cmd: GovCommand) -> Result<()> {
    let pid = ctx.lockup_pid;
    let client = ctx.connect::<Client>(pid)?;
    let authority = ctx.wallet()?;
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
    }
    Ok(())
}
