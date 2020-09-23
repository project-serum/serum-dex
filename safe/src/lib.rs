//! serum-safe defines the interface for the serum safe program.

#![cfg_attr(feature = "strict", deny(warnings))]

use solana_client_gen::prelude::*;

#[cfg_attr(feature = "client", solana_client_gen)]
pub mod instruction {
    use super::*;
    #[derive(serde::Serialize, serde::Deserialize)]
    pub enum SrmSafeInstruction {
        /// Initializes a safe instance for use.
        ///
        /// Similar to a token mint, this must be included in the same
        /// instruction that creates the account to initialize. Otherwise
        /// someone can take control of the account by calling initialize on it.
        ///
        /// Accounts:
        ///
        /// 0. `[writable]` Safe to initialize.
        /// 1. `[]`         Rent sysvar
        Initialize {
            /// The mint of the SPL token controlled by the safe, e.g., the SRM
            /// mint.
            mint: Pubkey,
            /// The priviledged account.
            authority: Pubkey,
            /// The nonce to use to create the Safe's derived-program address,
            /// which is used as the authority for the safe's token vault.
            nonce: u8,
        },
        /// DepositSrm creates deposit and vesting account, transferring tokens
        /// from the controlling token account to one owned by the SrmSafe
        /// program.
        ///
        /// Accounts:
        ///
        /// 0. `[writable]  Vesting account representing this deposit.
        ///                 The owner of this account is this program.
        ///                 It's data size is dynamic.
        /// 1. `[writable]` Depositor token account, transferring ownership
        ///                 *from*, itself to this program.
        /// 2. `[signer]`   The authority/owner/delegate of Accounts[1].
        /// 3. `[writable]` The program controlled token vault, transferring
        ///                 ownership *to*. The owner of this account is the
        ///                 program derived address with nonce set in the
        ///                 Initialize instruction.
        /// 4. `[]`         Safe instance.
        /// 5. `[]`         SPL token program.
        /// 6. `[]`         Rent sysvar.
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
        /// MintLockedSrm mints a "locked" token and sends it to the
        /// beneficiary's token account representing the locked NFT.
        ///
        /// The beneficiary's token account, should be the only account holding
        /// the locked NFT. Otherwise, the underlying token which the NFT
        /// is redeemable for might be slashed.
        ///
        /// Accounts:
        ///
        /// 0.  `[signer]`   Vesting account beneficiary.
        /// 1.  `[writable]` Vesting account to mint lSRM from.
        /// 2.  `[]`         Safe instance.
        /// 3.  `[]`         Safe's vault authority, a program derived address.
        /// 4.  `[]`         SPL token program.
        /// 5.  `[]`         Rent sysvar.
        /// ... `[writable]` Variable number of token mints, one for each NFT
        ///                  instance. The mint must *not* be initialized.
        /// ... `[writable]` Variable number of token accounts associated with
        ///                  each NFT mint.
        /// ... `[writable]` Variable number of NFT receipts each owned by this
        ///                  program and must *not* be initialized.
        ///
        /// Note that the trailing "variable" accounts are all "zipped"
        /// together. That is, they must be ordered in groups of three, each
        /// group representing the accounts required for a single locked NFT.
        MintLockedSrm {
            /// The *owner* of the SPL token account to send the funds to.
            /// It's currently assumed all NFT token accounts have the same
            /// owner that may or may not be equal to the vesting account
            /// beneficiary.
            token_account_owner: Pubkey,
        },
        /// BurnLockedSrm destroys the NFT pegged to the vesting account's
        /// deposit, so that subsequent withdrawals and lSRM issuance are
        /// unaffected by the outstanding NFT.
        ///
        /// Accounts:
        ///
        /// 0. `[signer]`   Owner of the token Account holding the NFT.
        /// 1. `[writable]` Token Account holding the NFT.
        /// 2. `[writable]` Token Mint representing the NFT issue.
        /// 3. `[writable]` Receipt proving validity of the NFT.
        /// 4. `[writable]` Vesting account owning the lSRM.
        /// 5. `[]`         SPL token program.
        ///
        BurnLockedSrm,
        /// WithdrawSrm withdraws the given amount from the given vesting
        /// account subject to a vesting schedule.
        ///
        /// Accounts:
        ///
        /// 0. `[signer]`   Vesting account's beneficiary.
        /// 1. `[writable]` Vesting account to withdraw from.
        /// 2. `[writable]` SRM token account to withdraw to.
        /// 3. `[writable]` Safe's token account vault from which we are
        ///                 transferring ownership of the SRM out of.
        /// 4. `[readonly]` Safe's vault authority, i.e., the program-derived
        ///                 address.
        /// 5  `[]`         Safe account.
        /// 6. `[]`         SPL token program.
        /// 7. `[]`         Clock sysvar.
        WithdrawSrm { amount: u64 },
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
        /// 0. `[signer]    Safe's authority.
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

// Define modules below so the macro output is in scope.
#[cfg(feature = "client")]
pub mod client_ext;
#[cfg(feature = "client")]
pub use client_ext::client;
#[cfg(feature = "client")]
pub use client_ext::instruction;

pub mod accounts;
pub mod error;
