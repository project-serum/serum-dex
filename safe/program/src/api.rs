use solana_sdk::account_info::AccountInfo;
#[cfg(feature = "program")]
use solana_sdk::info;
use solana_sdk::pubkey::Pubkey;

// todo: remove this shouldn't be necessary
#[cfg(not(feature = "program"))]
macro_rules! info {
    ($($i:expr),*) => { { ($($i),*) } };
}

pub fn initialize(accounts: &[AccountInfo], admin: Pubkey) {
    info!(format!("intialize with accounts {:?}, admin {:?}", accounts, admin).as_str());
}

pub fn slash(accounts: &[AccountInfo]) {
    // todo
}

pub fn deposit_srm(
    accounts: &[AccountInfo],
    user_spl_wallet_owner: Pubkey,
    slot_number: u64,
    amount: u64,
    lsrm_amount: u64,
) {
    info!("**********deposit SRM!");
}

pub fn withdraw_srm(accounts: &[AccountInfo], amount: u64) {
    info!("**********withdraw SRM!");
}

pub fn mint_locked_srm(accounts: &[AccountInfo], amount: u64) {
    info!("**********mint SRM!");
}

pub fn burn_locked_srm(accounts: &[AccountInfo], amount: u64) {
    info!("**********burn SRM!");
}
