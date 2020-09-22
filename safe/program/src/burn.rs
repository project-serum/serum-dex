use serum_safe::error::{SafeError, SafeErrorCode};
use serum_safe::pack::DynPack;
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::info;
use solana_sdk::program_error::ProgramError;
use solana_sdk::pubkey::Pubkey;
use spl_token::pack::Pack;

pub fn handler(program_id: &Pubkey, accounts: &[AccountInfo]) -> Result<(), SafeError> {
    info!("HANDLER: burn_locked_srm");

    Ok(())
}

fn access_control() -> Result<(), SafeError> {
    // todo
    Ok(())
}
