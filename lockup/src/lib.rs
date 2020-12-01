#![cfg_attr(feature = "strict", deny(warnings))]
#![allow(dead_code)]

use borsh::{BorshDeserialize, BorshSchema, BorshSerialize};
use serum_common::pack::*;
use solana_client_gen::prelude::*;

pub mod accounts;
pub mod error;

#[cfg_attr(feature = "client", solana_client_gen)]
pub mod instruction {
    use super::*;

    pub const TAG: u64 = 0x9c52b5632b5f74d2;

    #[derive(Debug, BorshSerialize, BorshDeserialize, BorshSchema)]
    pub enum LockupInstruction {
        /// Accounts:
        ///
        /// 0. `[writable]` Safe.
        /// 1. `[writable]` Whitelist.
        /// 2. `[]`         Rent sysvar.
        Initialize {
            /// The priviledged account.
            authority: Pubkey,
        },
        /// Accounts:
        ///
        /// 0. `[writable]  Vesting.
        /// 1. `[writable]` Depositor token account.
        /// 2. `[signer]`   The authority||owner||delegate of Accounts[1].
        /// 3. `[writable]` Vault.
        /// 4. `[]`         Safe.
        /// 7. `[]`         Token program.
        /// 8. `[]`         Rent sysvar.
        /// 9. `[]`         Clock sysvar.
        CreateVesting {
            /// The user who will own the SRM upon vesting.
            beneficiary: Pubkey,
            /// The unix timestamp at which point the entire deposit will
            /// be vested.
            end_ts: i64,
            /// The number of vesting periods for the account. For example,
            /// a vesting yearly over seven years would make this 7.
            period_count: u64,
            /// The amount to deposit into the vesting account.
            deposit_amount: u64,
            /// Vault signer nonce.
            nonce: u8,
        },
        /// Accounts:
        ///
        /// 0. `[signer]`   Beneficiary.
        /// 1. `[writable]` Vesting.
        /// 2. `[writable]` SPL token account to withdraw to.
        /// 3. `[writable]` Vault.
        /// 4. `[]`         Vault authority.
        /// 5  `[]`         Safe.
        /// 8. `[]`         SPL token program.
        /// 9. `[]`         Clock sysvar.
        // todo: rename
        Redeem { amount: u64 },
        /// Accounts:
        ///
        /// 0. `[signer]`   Beneficiary.
        /// 1. `[writable]` Vesting.
        /// 2. `[]`         Safe.
        /// 3. `[]`         Vault authority.
        /// 4. `[]`         Whitelisted program to invoke.
        /// 5. `[]`         Whitelist.
        ///
        /// All accounts below will be relayed to the whitelisted program.
        ///
        /// 6.  `[]`         Vault authority.
        /// 7.  `[writable]` Vault.
        /// 8.  `[writable]` Whitelisted target vault which will receive funds.
        /// 9.  `[]`         Whitelisted vault authority.
        /// 10. `[]`         Token program id.
        /// ..  `[writable]` Variable number of program specific accounts to
        ///                  relay to the program.
        WhitelistWithdraw {
            /// Amount of funds the whitelisted program is approved to
            /// transfer to itself. Must be less than or equal to the vesting
            /// account's whitelistable balance.
            amount: u64,
            /// Opaque instruction data to relay to the whitelisted program.
            instruction_data: Vec<u8>,
        },
        /// Accounts:
        ///
        /// Same as WhitelistWithdraw.
        WhitelistDeposit { instruction_data: Vec<u8> },
        /// Accounts:
        ///
        /// 0. `[signed]`   Safe authority.
        /// 1. `[]`         Safe account.
        /// 2. `[writable]` Whitelist.
        WhitelistAdd { entry: accounts::WhitelistEntry },
        /// Accounts:
        ///
        /// 0. `[signed]`   Safe authority.
        /// 1. `[]`         Safe account.
        /// 2. `[writable]` Whitelist.
        WhitelistDelete { entry: accounts::WhitelistEntry },
        /// Accounts:
        ///
        /// 0. `[signer]`   Current safe authority.
        /// 1. `[writable]` Safe instance.
        SetAuthority { new_authority: Pubkey },
        /// Accounts:
        ///
        /// 0. `[]` Vesting.
        /// 1. `[]` Clock sysvar.
        AvailableForWithdrawal,
    }
}

serum_common::packable!(instruction::LockupInstruction);
