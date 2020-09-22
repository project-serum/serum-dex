use serum_safe::error::SafeError;
//use serum_safe::pack::DynPack;
use solana_sdk::account_info::AccountInfo;
use solana_sdk::info;
use solana_sdk::pubkey::Pubkey;

pub fn handler(_program_id: &Pubkey, _accounts: &[AccountInfo]) -> Result<(), SafeError> {
    info!("HANDLER: burn_locked_srm");
    // todo
    access_control()?;
    Ok(())
}

fn access_control() -> Result<(), SafeError> {
    // todo
    Ok(())
}