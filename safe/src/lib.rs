//! serum-safe defines the interface for the serum safe program.

#![cfg_attr(feature = "strict", deny(warnings))]

use solana_client_gen::prelude::*;

#[cfg_attr(feature = "client", solana_client_gen)]
pub mod instruction {
    use super::*;
    #[derive(serde::Serialize, serde::Deserialize)]
    pub enum SrmSafeInstruction {
        /// Initialize instruction configures the safe with an admin that is
        /// responsible for slashing people who use their locked serum for
        /// invalid purposes.
        ///
        /// Similar to a mint, this must be included in the same instruction
        /// that creates the account to initialize. Otherwise someone
        /// can take control of the account by calling initialize on it.
        ///
        /// Accounts:
        ///
        /// 0. `[writable]` The SafeAccount to initialize.
        /// 1. `[]`         Rent sysvar
        Initialize {
            /// The mint of the SPL token to store in the safe, i.e., the
            /// SRM mint.
            mint: Pubkey,
            /// The owner of the admin account to set into the SafeAccount.
            /// This account has the power to slash deposits.
            authority: Pubkey,
            /// The nonce to use for the safe's spl vault authority program derived
            /// address.
            nonce: u8,
        },
        /// DepositSrm initializes the deposit, transferring tokens from the controlling SPL token
        /// account to one owned by the SrmSafe program.
        ///
        /// Accounts:
        ///
        /// 0. `[writable]  The VestingAccount representing this deposit. It is
        ///                 initialized with the data provided by the instruction.
        ///                 The owner of this account is the SrmSafe program.
        ///                 Note that it's data size is dynamic.
        /// 1. `[writable]` The depositor SRM SPL token account, transferring ownership *from*,
        ///                 itself to this program.
        /// 2. `[signer]`   The authority/owner/delegate of Accounts[1].
        /// 3. `[writable]` The SrmSafe SPL SRM vault, transferring ownership *to*.
        ///                 The owner of this account is the SrmSafe program.
        /// 4. `[]`         The SafeAccount instance.
        /// 5. `[]`         SPL token program.
        /// 6. `[]`         The rent sysvar.
        #[cfg_attr(feature = "client", create_account(..))]
        DepositSrm {
            /// The beneficiary of the vesting account, i.e.,
            /// the user who will own the SRM upon vesting.
            vesting_account_beneficiary: Pubkey,
            /// The Solana slot number at which point a vesting amount unlocks.
            vesting_slots: Vec<u64>,
            /// The amount of SRM to release for each vesting_slot.
            vesting_amounts: Vec<u64>,
        },
        /// MintLockedSrm mints an lSRM token and sends it to the depositor's lSRM SPL account,
        /// adjusting the vesting account's metadata as needed--increasing the amount of
        /// lSRM minted so that subsequent withdrawals will be affected by any outstanding
        /// locked srm associated with a vesting account.
        ///
        /// Accounts:
        ///
        /// 0.  `[signer]`   The vesting account beneficiary.
        /// 1.  `[writable]` The vesting account to mint lSRM from.
        /// 3.  `[]`         SPL token program.
        /// 4.  `[]`         The rent sysvar.
        /// ... `[writable]` A variable number of lSRM SPL mints one for each NFT
        ///                  instance of lSRM. The mint must be uninitialized.
        /// ... `[writable]` A variable number of lSRM receipts, one for each lSRM
        ///                  NFT, each owned by this program and given uninitialized.
        ///
        MintLockedSrm,
        /// BurnLockedSrm destroys the lSRM associated with the vesting account, so that
        /// subsequent withdrawals and lSRM issuance are unaffected by the outstanding
        /// coin.
        ///
        /// Accounts:
        ///
        /// 0. `[signer]`   The SPL Mint representing the lSRM NFT.
        /// 1. `[writable]` The vesting account owning the lSRM.
        /// 2. `[writable]` The lSRM receipt proving validity of the lSRM.
        ///
        BurnLockedSrm,
        /// WithdrawSrm withdraws the given amount from the given vesting account.
        ///
        /// Accounts:
        ///
        /// 0. `[signer]`   The vesting account's beneficiary.
        /// 1. `[writable]` The vesting account to withdraw from.
        /// 2. `[writable]` The SRM SPL token account to withdraw to.
        /// 3. `[writable]` The Safe's SPL account vault from which we are transferring
        ///                 ownership of the SRM out of.
        /// 4  `[]`         The SrmSafe account.
        /// 5. `[]`         SPL token program.
        /// 4. `[]`         Clock sysvar.
        WithdrawSrm { amount: u64 },
        /// Sets the new authority for the safe instance. If set to all zeroes, then
        /// authority is forever lost.
        ///
        /// 0. `[signer]`   The current safe authority.
        /// 1. `[writable]` The SafeAccount instance.
        SetAuthority { new_authority: Pubkey },
        /// Migrate sends all the SRM locked by this safe to a new address. This
        /// should be used as a temporary measure to ship a v1 of this program,
        /// allowing new features to be considered and developed. In the future
        /// the authority should be disabled, or moved to a more robust governance
        /// mechanism.
        ///
        /// Accounts:
        ///
        /// 0. `[signer]    The SafeAccount's authority.
        /// 1. `[writable]` The Safe's SPL account vault from which we are transferring
        ///                 The SPL token out of.
        /// 2  `[writable]` The SrmSafe account.
        /// 3. `[]`         The SPL token program.
        Migrate,
    }
}

// Define modules below so the macro output is in scope.
#[cfg(feature = "client")]
pub mod client_ext;
#[cfg(feature = "client")]
pub use client_ext::client;
#[cfg(feature = "client")]
pub use client_ext::instruction;

pub mod accounts;
pub mod error;
pub mod pack;
