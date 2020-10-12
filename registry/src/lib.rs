#![cfg_attr(feature = "strict", deny(warnings))]
#![allow(dead_code)]

use borsh::{BorshDeserialize, BorshSchema, BorshSerialize};
use serum_common::pack::*;
use solana_client_gen::prelude::*;

pub mod access_control;
pub mod accounts;
pub mod error;

// TODO: add pool accounts once it's ready.

#[cfg_attr(feature = "client", solana_client_gen)]
pub mod instruction {
    use super::*;
    #[derive(BorshSerialize, BorshDeserialize, BorshSchema)]
    pub enum RegistryInstruction {
        /// Initializes the registry instance for use.
        ///
        /// Accounts:
        ///
        /// 0. `[writable]` Registrar to initialize.
        /// 1. `[]`         SRM "stake-intent" vault.
        /// 2. `[]`         MSRM "stake-intent" vault.
        /// 3. `[]`         Rent sysvar.
        Initialize {
            /// The priviledged account.
            authority: Pubkey,
            /// Nonce for deriving the vault authority address.
            nonce: u8,
            /// Number of seconds that must pass for a withdrawal to complete.
            withdrawal_timelock: i64,
            /// Number of seconds *in addition* to the `withdrawal_timelock` after
            /// which an Entity becomes "deactivated".  The deactivation
            /// countdown starts immediately once a node's stake amount is less
            /// than the reward_activation_threshold.
            deactivation_timelock_premium: i64,
            /// The amount of tokens that must be staked for an entity to be
            /// eligible for rewards.
            reward_activation_threshold: u64,
        },
        /// RegisterCapability registers a node capability for reward
        /// collection, or overwrites an existing capability (e.g., on fee
        /// change).
        ///
        /// Accounts:
        ///
        /// 0. `[signer]`   Registrar authority.
        /// 1. `[writable]` Registrar instance.
        RegisterCapability {
            /// The identifier to assign this capability.
            capability_id: u8,
            /// Capability fee in bps. The amount to pay a node for an
            /// instruction fulfilling this duty.
            capability_fee_bps: u32,
        },
        /// CreateEntity initializes the new "node" with the Registry,
        /// designated "inactive".
        ///
        /// Accounts:
        ///
        /// 0. `[writable]` Entity account.
        /// 1. `[signer]`   Leader of the node.
        /// 2. `[]`         Registrar.
        /// 3. `[]`         Rent sysvar.
        CreateEntity {
            /// The Serum ecosystem duties a Node performs to earn extra
            /// performance based rewards, for example, cranking.
            capabilities: u32,
            /// Type of governance backing the `Entity`.
            stake_kind: accounts::StakeKind,
        },
        /// UpdateEntity updates the leader and capabilities of the node entity.
        ///
        /// Accounts:
        ///
        /// 0. `[writable]` Entity account.
        /// 1. `[signer]`   Leader of the entity.
        UpdateEntity { leader: Pubkey, capabilities: u32 },
        /// Joins the entity by creating a membership account.
        ///
        /// Accounts:
        ///
        /// 0. `[writable]` Member account being created.
        /// 1. `[]`         Entity account to stake to.
        /// 2. `[]`         Registrar.
        /// 3. `[]`         Rent sysvar.
        JoinEntity {
            /// The owner of this entity account. Must sign off when staking and
            /// withdrawing.
            beneficiary: Pubkey,
            /// An account that can withdrawal or stake on the beneficiary's
            /// behalf.
            delegate: Pubkey,
            /// Watchtower authority assigned to the resulting member account.
            watchtower: accounts::Watchtower,
        },
        /// Accounts:
        ///
        /// 0. `[writable]` Member account.
        /// 1. `[signed]`   Beneficiary of the member account.
        UpdateMember {
            watchtower: Option<accounts::Watchtower>,
            /// Delegate can only be updated if the delegate's book balance is 0.
            delegate: Option<Pubkey>,
        },
        /// Accounts:
        ///
        /// Lockup whitelist relay account interface:
        ///
        /// 0. `[]`         Member account's delegate owner. If not a delegated
        ///                 instruction, then a dummy account.
        /// 1. `[writable]` The depositing token account (sender).
        /// 2. `[writable]` Vault (receiver).
        /// 3. `[signer]`   Owner/delegate of the depositing token account.
        /// 4. `[]`         SPL token program.
        ///
        /// Program specific.
        ///
        /// 5. `[writable]` Member account responsibile for the stake.
        /// 6. `[signer]`   Beneficiary of the Member account being staked.
        /// 7. `[writable]` Entity account to stake to.
        /// 8. `[]`         Registrar.
        /// 9. `[]`         Clock.
        StakeIntent {
            amount: u64,
            mega: bool,
            delegate: bool,
        },
        /// Accounts:
        ///
        /// Same as StakeIntent.
        StakeIntentWithdrawal {
            amount: u64,
            mega: bool,
            delegate: bool,
        },
        /// Transfers the stake intent funds into the staking pool.
        ///
        ///
        TransferStakeIntent {
            amount: u64,
            mega: bool,
            delegate: bool,
        },
        /// Accounts:
        ///
        /// Same as StakeIntent, substituting Accounts[1] for the pool's vault.
        ///
        /// TODO: append pool specific accounts once we know the interface.
        Stake {
            amount: u64,
            mega: bool,
            delegate: bool,
        },
        /// Initiates a stake withdrawal. Funds are locked up until the
        /// withdrawl timelock passes.
        ///
        /// Accounts:
        ///
        /// 0. `[writable]  PendingWithdrawal account to initialize.
        /// 1  `[signed]`   Benficiary of the Stake account.
        /// 2. `[writable]` The Member account to withdraw from.
        /// 3. `[writable]` Entity the Stake is associated with.
        /// 4. `[writable]` Registrar.
        /// 5. `[]`         Rent acc info.
        /// 6. `[signed]`   Owner of the staking pool token account to redeem.
        ///
        /// Delegate only.
        ///
        /// 7. `[signed]?`  Delegate owner of the Member account.
        // TODO: the staking pool token should be burned here so that it can't
        //       be used during the withdrawal timelock period.
        /// 7. `[writable]` Staking pool token.
        /// 8. `[writable]` Staking pool token mint.
        StartStakeWithdrawal {
            amount: u64,
            mega: bool,
            delegate: bool,
        },
        /// Completes the pending withdrawal once the timelock period passes.
        ///
        /// Accounts:
        ///
        /// 0. `[writable]  PendingWithdrawal account to complete.
        /// 1. `[signed]`   Beneficiary/delegate of the member account.
        /// 2. `[writable]` Member account to withdraw from.
        /// 3. `[writable]` Entity account the member is associated with.
        /// 4. `[]`         SPL token program (SRM).
        /// 5. `[]`         SPL mega token program (MSRM).
        /// 6. `[writable]` SRM token account to send to upon redemption
        /// 7. `[writable]` MSRM token account to send to upon redemption
        EndStakeWithdrawal,
        /// Donates funds into the staking pool for reward distribution. Anyone
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
    }
}

serum_common::packable!(instruction::RegistryInstruction);
