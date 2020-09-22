use serum_safe::error::SafeError;
use solana_sdk::account_info::AccountInfo;
use solana_sdk::info;
use solana_sdk::pubkey::Pubkey;

pub fn handler(
    _program_id: &Pubkey,
    _accounts: &[AccountInfo],
    _new_authority: &Pubkey,
) -> Result<(), SafeError> {
    info!("HANDLER: set_authority");

    access_control()?;
    Ok(())
}

fn access_control() -> Result<(), SafeError> {
    // todo
    Ok(())
}
