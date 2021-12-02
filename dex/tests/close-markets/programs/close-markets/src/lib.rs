use anchor_lang::prelude::*;
use anchor_spl::token::{Token, TokenAccount, Mint};
use anchor_spl::dex;

pub use serum_dex;

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

pub const SERUM_MARKET_SEED: &[u8; 12] = b"serum_market";
pub const REQUEST_QUEUE_SEED: &[u8; 13] = b"request_queue";
pub const COIN_VAULT_SEED: &[u8; 10] = b"coin_vault";
pub const PC_VAULT_SEED: &[u8; 8] = b"pc_vault";
pub const OPEN_ORDERS_SEED: &[u8; 11] = b"open_orders";

// 388 for marketstate + 32 * 3 = 96 for pubkey + 992 for padding = 1476
pub const SERUM_MARKET_SPACE: usize = 1476;
pub const REQUEST_QUEUE_SPACE: usize = 5120 + 12;
pub const OPEN_ORDERS_SPACE: usize = 3228;

pub const COIN_LOT_SIZE: u64 = 10_000;
pub const PC_LOT_SIZE: u64 = 10;
pub const PC_DUST_THRESHOLD: u64 = 100;

#[program]
pub mod close_markets {
    use super::*;
    pub fn initialize_market(ctx: Context<InitializeMarket>, bumps: Bumps) -> ProgramResult {
        let vault_signer_nonce = bumps.vault_signer as u64;

        let prune_auth = &mut *ctx.accounts.prune_auth;
        prune_auth.payer = ctx.accounts.payer.key();
        prune_auth.bumps = bumps;

        dex::initialize_market(
            ctx.accounts.as_initialize_serum_market(),
            COIN_LOT_SIZE,
            PC_LOT_SIZE,
            vault_signer_nonce,
            PC_DUST_THRESHOLD,
        )?;

        Ok(())
    }

    pub fn close_market(ctx: Context<CloseMarket>) -> ProgramResult {
        let ix = serum_dex::instruction::close_market(
            &ctx.accounts.dex_program.key(),
            &ctx.accounts.serum_market.key(),
            &ctx.accounts.request_queue.key(),
            &ctx.accounts.event_queue.key(),
            &ctx.accounts.bids.key(),
            &ctx.accounts.asks.key(),
            &ctx.accounts.prune_auth.key(),
            &ctx.accounts.payer.key(),
        )?;

        let seeds = &[b"prune_auth".as_ref(), &[ctx.accounts.prune_auth.bumps.prune_auth]];
        let signer = &[&seeds[..]];

        solana_program::program::invoke_signed(
            &ix,
            &ToAccountInfos::to_account_infos(ctx.accounts),
            signer,
        )?;

        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(bumps: Bumps)]
pub struct InitializeMarket<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(
        init,
        seeds = [b"prune_auth".as_ref()],
        bump = bumps.prune_auth,
        payer = payer
    )]
    pub prune_auth: Box<Account<'info, PruneAuth>>,
    #[account(
        init,
        mint::decimals = 6,
        mint::authority = payer,
        seeds = [b"usdc_mint".as_ref()],
        bump = bumps.usdc_mint,
        payer = payer
    )]
    pub usdc_mint: Box<Account<'info, Mint>>,
    #[account(
        init,
        mint::decimals = 6,
        mint::authority = payer,
        seeds = [b"serum_mint".as_ref()],
        bump = bumps.serum_mint,
        payer = payer
    )]
    pub serum_mint: Box<Account<'info, Mint>>,
    #[account(
        init,
        seeds = [SERUM_MARKET_SEED.as_ref()],
        bump = bumps.serum_market,
        space = SERUM_MARKET_SPACE, 
        payer = payer,
        owner = dex_program.key(
    ))]
    pub serum_market: UncheckedAccount<'info>,
    #[account(
        init,
        seeds = [REQUEST_QUEUE_SEED.as_ref()],
        bump = bumps.request_queue,
        space = REQUEST_QUEUE_SPACE,
        payer = payer,
        owner = dex_program.key(
    ))]
    pub request_queue: UncheckedAccount<'info>,
    #[account(
        init,
        token::mint = serum_mint,
        token::authority = vault_signer,
        seeds = [COIN_VAULT_SEED.as_ref()],
        bump = bumps.coin_vault,
        payer = payer
    )]
    pub coin_vault: Box<Account<'info, TokenAccount>>,
    #[account(
        init,
        token::mint = usdc_mint,
        token::authority = vault_signer,
        seeds = [PC_VAULT_SEED.as_ref()],
        bump = bumps.pc_vault,
        payer = payer
    )]
    pub pc_vault: Box<Account<'info, TokenAccount>>,
    // TODO probably no way to verify these seeds since it uses a different heuristic
    pub vault_signer: UncheckedAccount<'info>,
    // These shouldn't need to be signers
    #[account(mut)]
    pub event_queue: UncheckedAccount<'info>,
    #[account(mut)]
    pub bids: UncheckedAccount<'info>,
    #[account(mut)]
    pub asks: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    // TODO check this via anchor-spl?
    pub dex_program: UncheckedAccount<'info>,
    pub rent: Sysvar<'info, Rent>,
}

impl<'info> InitializeMarket<'info> {
    pub fn as_initialize_serum_market(&self) -> CpiContext<'_, '_, '_, 'info, dex::InitializeMarket<'info>> {
        let cpi_accounts = dex::InitializeMarket {
            market: self.serum_market.to_account_info(),
            req_q: self.request_queue.to_account_info(),
            event_q: self.event_queue.to_account_info(),
            bids: self.bids.to_account_info(),
            asks: self.asks.to_account_info(),
            coin_vault: self.coin_vault.to_account_info(),
            pc_vault: self.pc_vault.to_account_info(),
            coin_mint: self.serum_mint.to_account_info(),
            pc_mint: self.usdc_mint.to_account_info(),
            rent: self.rent.to_account_info(),
        };
        let cpi_ctx = CpiContext::new(self.dex_program.to_account_info(), cpi_accounts);
        // prune_auth will be the open orders auth and prune auth
        cpi_ctx.with_remaining_accounts(vec![
            self.prune_auth.to_account_info() ,self.prune_auth.to_account_info()
            ])
    }
}

#[derive(Accounts)]
pub struct CloseMarket<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(
        seeds = [b"prune_auth".as_ref()],
        bump = prune_auth.bumps.prune_auth,
    )]
    pub prune_auth: Box<Account<'info, PruneAuth>>,
    #[account(
        mut,
        seeds = [SERUM_MARKET_SEED.as_ref()],
        bump = prune_auth.bumps.serum_market,
    )]
    pub serum_market: UncheckedAccount<'info>,
    #[account(mut)]
    pub request_queue: UncheckedAccount<'info>,
    #[account(mut)]
    pub event_queue: UncheckedAccount<'info>,
    #[account(mut)]
    pub bids: UncheckedAccount<'info>,
    #[account(mut)]
    pub asks: UncheckedAccount<'info>,
    pub dex_program: UncheckedAccount<'info>,
}


#[account]
#[derive(Default)]
pub struct PruneAuth {
    pub payer: Pubkey,
    pub bumps: Bumps,
}

#[derive(AnchorSerialize, AnchorDeserialize, Default, Clone)]
pub struct Bumps {
    pub prune_auth: u8,
    pub usdc_mint: u8,
    pub serum_mint: u8,
    pub serum_market: u8,
    pub request_queue: u8,
    pub coin_vault: u8,
    pub pc_vault: u8,
    pub vault_signer: u8, // This one follows a different bump discovery formula
}
