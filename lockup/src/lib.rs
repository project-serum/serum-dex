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
    #[derive(BorshSerialize, BorshDeserialize, BorshSchema)]
    pub enum LockupInstruction {
        /// Initializes a safe instance for use.
        ///
        /// Similar to a token mint, this must be included in the same
        /// instruction that creates the Safe account to initialize. Otherwise
        /// someone can take control of the account by calling initialize on it.
        ///
        /// Accounts:
        ///
        /// 0. `[writable]` Safe to initialize.
        /// 1. `[writable]` Whitelist to initialize.
        /// 2. `[]`         Vault.
        /// 4. `[]`         Mint of the SPL token controlled by the safe.
        /// 5. `[]`         Rent sysvar.
        Initialize {
            /// The priviledged account.
            authority: Pubkey,
            /// The nonce to use to create the Safe's derived-program address,
            /// which is used as the authority for the safe's token vault.
            nonce: u8,
        },
        /// CreateVesting initializes a vesting account, transferring tokens
        /// from the controlling token account to one owned by the program.
        /// Anyone with funds to deposit can invoke this instruction.
        ///
        /// Accounts:
        ///
        /// 0. `[writable]  Vesting account representing this deposit.
        /// 1. `[writable]` Depositor token account, transferring ownership
        ///                 *from*, itself to this program's vault.
        /// 2. `[signer]`   The authority||owner||delegate of Accounts[1].
        /// 3. `[writable]` The program controlled token vault, transferring
        ///                 ownership *to*. The owner of this account is the
        ///                 program derived address with nonce set in the
        ///                 Initialize instruction.
        /// 4. `[]`         Safe instance.
        /// 5. `[writable]` Token mint representing the lSRM receipt.
        /// 6. `[]`         Safe's vault authority, a program derived address.
        ///                 The mint authority.
        /// 7. `[]`         SPL token program.
        /// 8. `[]`         Rent sysvar.
        /// 9. `[]`         Clock sysvar.
        CreateVesting {
            /// The beneficiary of the vesting account, i.e.,
            /// the user who will own the SRM upon vesting.
            beneficiary: Pubkey,
            /// The unix timestamp at which point the entire deposit will
            /// be vested.
            end_ts: i64,
            /// The number of vesting periods for the account. For example,
            /// a vesting yearly over seven years would make this 7.
            period_count: u64,
            /// The amount to deposit into the vesting account.
            deposit_amount: u64,
        },
        /// Claim is an instruction for one time use by the beneficiary of a
        /// Vesting account. It mints a non-fungible SPL token and sends it
        /// to an account owned by the beneficiary as a of receipt SRM locked.
        ///
        /// The beneficiary, and only the beneficiary, can redeem this token
        /// in exchange for the underlying asset as soon as the account vests.
        ///
        /// Accounts:
        ///
        /// 0. `[signer]`   Vesting account beneficiary.
        /// 1. `[writable]` Vesting account.
        /// 2. `[]`         Safe instance.
        /// 3. `[]`         Safe's vault authority, a program derived address.
        /// 4. `[]`         SPL token program.
        /// 5. `[writable]` Token mint representing the lSRM receipt.
        /// 6  `[writable]` Token account associated with the mint.
        Claim,
        /// Reedeem exchanges the given `amount` of non-fungible, claimed
        /// receipt tokens for the underlying locked SRM, subject to the
        /// Vesting account's vesting schedule.
        ///
        /// Accounts:
        ///
        /// 0. `[signer]`   Vesting account's beneficiary.
        /// 1. `[writable]` Vesting account to withdraw from.
        /// 2. `[writable]` SPL token account to withdraw to.
        /// 3. `[writable]` Safe's token account vault from which we are
        ///                 transferring ownership of the SRM out of.
        /// 4. `[]`         Safe's vault authority, i.e., the program-derived
        ///                 address.
        /// 5  `[]`         Safe account.
        /// 6. `[writable]` NFT token being redeemed.
        /// 7. `[writable]` NFT mint to burn the token being redeemed.
        /// 8. `[]`         SPL token program.
        /// 9. `[]`         Clock sysvar.
        Redeem { amount: u64 },
        /// Invokes an opaque instruction on a whitelisted program,
        /// giving it delegate access to send `amount` funds to itself.
        ///
        /// For example, a user could call this with a staking program
        /// instruction to send locked SRM to it without custody ever leaving
        /// an on-chain program.
        ///
        /// Accounts:
        ///
        /// 0. `[signer]`   Vesting beneficiary.
        /// 1. `[writable]` Vesting.
        /// 2. `[]`         Safe (containing the nonce).
        /// 3. `[]`         Safe vault authority.
        /// 4. `[]`         Whitelisted program to invoke.
        /// 5. `[]`         Whitelist.
        ///
        /// All accounts below will be relayed to the whitelisted program.
        ///
        /// 6.  `[]`         Safe vault authority.
        /// 7.  `[writable]` Safe vault.
        /// 8.  `[writable]` Vault which will receive funds.
        /// 9.  `[]`         Whitelisted vault authority.
        /// 10. `[]`         Token program id.
        /// ..  `[writable]` Variable number of program specific accounts to
        ///                  relay to the program, along with the above
        ///                  whitelisted accounts and Safe vault.
        WhitelistWithdraw {
            /// Amount of funds the whitelisted program is approved to
            /// transfer to itself. Must be less than or equal to the vesting
            /// account's whitelistable balance.
            amount: u64,
            /// Opaque instruction data to relay to the whitelisted program.
            instruction_data: Vec<u8>,
        },
        /// Makes an opaque cross program invocation to a whitelisted program.
        /// It's expected the CPI will deposit funds into the Safe's vault.
        ///
        /// Accounts:
        ///
        /// Same as WhitelistWithdraw.
        WhitelistDeposit { instruction_data: Vec<u8> },
        /// Adds the given entry to the whitelist.
        ///
        /// Accounts:
        ///
        /// 0. `[signed]`   Safe authority.
        /// 1. `[]`         Safe account.
        /// 2. `[writable]` Whitelist.
        WhitelistAdd {
            entry: crate::accounts::WhitelistEntry,
        },
        /// Removes the given entry from the whitelist.
        ///
        /// Accounts:
        ///
        /// 0. `[signed]`   Safe authority.
        /// 1. `[]`         Safe account.
        /// 2. `[writable]` Whitelist.
        WhitelistDelete {
            entry: crate::accounts::WhitelistEntry,
        },
        /// Sets the new authority for the safe instance.
        ///
        /// 0. `[signer]`   Current safe authority.
        /// 1. `[writable]` Safe instance.
        SetAuthority { new_authority: Pubkey },
        /// Migrate sends all the SRM locked by this safe to a new address. This
        /// should be used as a temporary measure to ship a v1 of this program,
        /// allowing new features to be considered and developed.
        ///
        /// In the future the authority should be disabled, e.g., set to the
        /// zero key, or moved to a more robust governance mechanism.
        ///
        /// Accounts:
        ///
        /// 0. `[signer]`   Safe's authority.
        /// 1  `[writable]` Safe account.
        /// 2. `[writable]` Safe's token vault from which we are transferring
        ///                 all tokens out of.
        /// 3. `[readonly]` Safe's vault authority, i.e., the program derived
        ///                 address.
        /// 4. `[writable]` Token account to receive the new tokens.
        /// 5. `[]`         SPL token program.
        Migrate,
    }
}

serum_common::packable!(instruction::LockupInstruction);
