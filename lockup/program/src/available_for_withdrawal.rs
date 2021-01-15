use crate::common::access_control;
use serum_common::pack::Pack;
use serum_lockup::accounts::Vesting;
use serum_lockup::error::LockupError;
use solana_program::msg;
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::pubkey::Pubkey;

// Convenience instruction for UI's.
pub fn handler(_program_id: &Pubkey, accounts: &[AccountInfo]) -> Result<(), LockupError> {
    let acc_infos = &mut accounts.iter();

    let vesting_acc_info = next_account_info(acc_infos)?;
    let clock_acc_info = next_account_info(acc_infos)?;

    let mut d: &[u8] = &vesting_acc_info.try_borrow_data()?;
    let vesting = Vesting::unpack(&mut d)?;
    let clock = access_control::clock(clock_acc_info)?;

    let available = vesting.available_for_withdrawal(clock.unix_timestamp);
    // Log as string so that JS can read as a BN.
    msg!(&format!("{{ \"result\": \"{}\" }}", available));

    Ok(())
}
