//! serum-safe defines the interface for the serum safe program.

#![cfg_attr(feature = "strict", deny(warnings))]

use serum_common::pack::*;
use solana_client_gen::prelude::*;

#[cfg_attr(feature = "client", solana_client_gen(ext))]
pub mod instruction {
    use super::*;
    #[derive(serde::Serialize, serde::Deserialize)]
    pub enum RegistryInstruction {
        /// Initializes the registry instance for use. Anyone can invoke this
        /// instruction so it should be run in the same transaction as the
        /// create_account instruction for the Registry instance.
        ///
        /// Accounts:
        ///
        /// 0. `[writable]` Registry to initialize.
        /// 1. `[]`         Mint of the token to pay rewards with (SRM).
        /// 2. `[]`         Mint of the "Mega" token to pay rewards with (MSRM).
        /// 3. `[]`         Rent sysvar.
        Initialize {
            /// The priviledged account.
            authority: Pubkey,
            /// The nonce used to create the Registry's program-derived address,
            /// which owns all token vaults.
            nonce: u8,
        },
        /// SetRewards sets the rewards program to use.
        ///
        /// Accounts:
        ///
        /// 0. `[signer]`   Registry authority.
        /// 1. `[writable]` Registry.
        SetRewards {
            /// The Rewards program id.
            rewards: Pubkey,
            /// Account for retrieving the return value for a rewards
            /// calculation. Should already have been created.
            ///
            /// (Hack because Solana doesn't have return values. Remove this
            /// once that changes.)
            rewards_return_value: Pubkey,
        },
        /// Transfers rewards in the *non*-mega token to the given address.
        ///
        /// Accounts:
        ///
        /// 0  `[]`         Rewards program.
        /// 1. `[writable]` Rewards ReturnValue account.
        /// 2. `[writable]` Registry token vault.
        /// 3. `[]`         Registry token vault authority.
        /// 4. `[signed]`   Beneficiary of the Stake account to collect rewards.
        /// 5. `[writable]` Token account to send rewards to.
        /// 6. `[]`         Stake account from which to get rewards.
        /// 7. `[]`         Entity the stake account is associated with.
        /// 8. `[writable]` Registry instance.
        /// 9. `[]`         SPL token program (SRM).
        CollectRewards,
        /// Donates funds into the registry for reward distribution. Anyone
        /// can invoke this instruction. Only the non-mega token can be donated.
        ///
        /// Accounts:
        ///
        /// 0. `[signer]`   Owner of the account sending the funds.
        /// 1. `[writable]` Account from which to send the funds.
        /// 2. `[writable]` Program controlled token vault to transfer funds
        ///                 into.
        /// 3. `[]`         Registry instance, holding the nonce to calculate
        ///                 the program-derived-address.
        /// 4. `[]`         SPL token program.
        Donate {
            /// The amount to deposit.
            amount: u64,
        },
        /// CreateEntity initializes the new "node" with the Registry, allowing
        /// addresses to stake with it and collect rewards.
        ///
        /// Accounts:
        ///
        /// 0. `[writable]` Entity account.
        /// 1. `[signer]`   Leader of the node.
        /// 2. `[]`         Rent sysvar.
        #[cfg_attr(feature = "client", create_account(*entity::SIZE))]
        CreateEntity {
            capabilities: u32,
            stake_kind: crate::accounts::StakeKind,
        },
        /// UpdateEntity updates the capabilities of the node entity.
        ///
        /// Accounts:
        ///
        /// 0. `[writable]` Entity account.
        /// 1. `[signer]`   Leader of the node.
        UpdateEntity { capabilities: u32 },
        /// RegisterCapability adds a node capability for reward collection,
        /// or overwrites an existing capability (e.g., on program upgrade).
        ///
        /// Accounts:
        ///
        /// 0. `[signer]`   Registry authority.
        /// 1. `[writable]` Registry instance.
        RegisterCapability {
            /// The identifier to assign this capability.
            capability_id: u8,
            /// The external address used to calculate rewards for this
            /// capability.
            capability_program: Pubkey,
        },
        /// Stake deposits funds into a registered node entity pool,
        /// initializing the given beneficiary as a staker, if it's not already
        /// initialized.
        ///
        /// Accounts:
        ///
        /// 0. `[writable]` Stake account.
        /// 1. `[signed]`   Owner of the account to send funds from.
        /// 2. `[writable]` Token account containing the funds.
        /// 3. `[]`         Registry instance.
        /// 4. `[writable]` Program controlled token vault to transfer funds
        ///                 into.
        /// 5. `[writable]` Entity account to stake with.
        /// 6. `[]`         SPL token program.
        #[cfg_attr(feature = "client", create_account(*stake::SIZE))]
        Stake {
            /// The amount to stake.
            amount: u64,
            /// The key to associate wtih this deposit.
            beneficiary: Pubkey,
            /// True iff the token being transferred is the mega token.
            is_mega: bool,
        },
        /// StakeLocked is the same as the `Stake` instruction, but using
        /// the locked token minted from the Serum Safe.
        ///
        /// Accounts:
        ///
        /// 0. `[signed]`   Owner of the account to send funds from.
        /// 1. `[writable]` Token account containing the funds.
        /// 2. `[writable]` Stake account.
        /// 3. `[]`         Registry instance.
        /// 4. `[writable]` Program controlled token vault to transfer funds
        ///                 into.
        /// 5. `[writable]` Entity account to stake with.
        /// 6. `[]`         SPL token program.
        StakeLocked {
            amount: u64,
            beneficiary: Pubkey,
            // Assumes mega serum can't be locked.
        },
        /// Deposits more funds into a given staking account.
        ///
        /// Accounts:
        ///
        /// 0. `[signed]`   Owner of the account to send funds from.
        /// 1. `[writable]` Token account containing the funds to send from.
        /// 2. `[writable]` Stake account to add to.
        /// 3. `[]`         Registry instance.
        /// 4. `[writable]` Program controlled token vault to transfer funds
        ///                 into.
        /// 5. `[writable]` Entity account the stake is associated with.
        /// 6. `[]`         SPL token program.
        AddStake { amount: u64 },
        /// Initiates a stake withdrawal. Funds are locked up until the
        /// withdrawl timelock passes.
        ///
        /// Accounts:
        ///
        /// 0  `[signed]`   Benficiary of the Stake account.
        /// 1. `[writable]` The Stake account to withdraw from.
        /// 2. `[writable]` Entity the Stake is associated with.
        InitiateStakeWithdrawal { amount: u64, mega_amount: u64 },
        /// Completes the initiated withdrawal.
        ///
        /// Accounts:
        ///
        /// 0. `[signed]`   Beneficiary of the Stake account.
        /// 1. `[writable]` Stake account to withdraw from.
        /// 2. `[writable]` Entity the Stake is associated with.
        /// 3. `[writable]` Program controlled token vault to transfer funds
        ///                 into.
        /// 4. `[]`         Registry instance.
        /// 5. `[]`         SPL token program (SRM).
        /// 6. `[]`         SPL mega token program (MSRM).
        /// 7. `[writable]` The token account to send funds to.
        /// 8. `[writable]` The mega token account to send funds to.
        CompleteStakeWithdrawal {
            // True if we want to withdraw the normal token out.
            is_token: bool,
            // True if we want to wtihdraw the mega token out.
            is_mega: bool,
        },
    }
}

#[cfg(feature = "client")]
pub mod client_ext;
#[cfg(feature = "client")]
pub use client_ext::client;
#[cfg(feature = "client")]
pub use client_ext::instruction;

pub mod accounts;
pub mod error;
pub mod rewards;

serum_common::packable!(crate::instruction::RegistryInstruction);
