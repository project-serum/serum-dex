//! mod accounts defines the storage layout for the accounts used by this program.

mod safe_account;
mod vesting_account;
mod vault;

pub use safe_account::{SafeAccount, Whitelist};
pub use vesting_account::VestingAccount;
pub use vault::SrmVault;
