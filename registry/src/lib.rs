//! serum-safe defines the interface for the serum safe program.

#![cfg_attr(feature = "strict", deny(warnings))]

use serum_common::pack::*;
use solana_client_gen::prelude::*;

// TODO: add pool accounts once it's ready.

#[cfg_attr(feature = "client", solana_client_gen(ext))]
pub mod instruction {
    use super::*;
    #[derive(serde::Serialize, serde::Deserialize)]
    pub enum RegistryInstruction {
        /// Initializes the registry instance for use. Anyone can invoke this
        /// instruction so it should be run in the same transaction as the
        /// create_account instruction.
        ///
        /// Accounts:
        ///
        /// 0. `[writable]` Registrar to initialize.
        /// 1. `[]`         Rent sysvar.
        #[cfg_attr(feature = "client", create_account(*registrar::SIZE))]
        Initialize {
            /// The priviledged account.
            authority: Pubkey,
            /// Number of slots that must pass for a withdrawal to complete.
            withdrawal_timelock: u64,
        },
        /// RegisterCapability registers a node capability for reward collection,
        /// or overwrites an existing capability (e.g., on fee change).
        ///
        /// Accounts:
        ///
        /// 0. `[signer]`   Registrar authority.
        /// 1. `[writable]` Registrar instance.
        RegisterCapability {
            /// The identifier to assign this capability.
            capability_id: u8,
            /// Capability fee in bps. The amount to pay a node for an instruction fulfilling
            /// this duty.
            capability_fee_bps: u32,
        },
        /// CreateEntity initializes the new "node" with the Registry, designated "inactive".
        ///
        /// Accounts:
        ///
        /// 0. `[writable]` Entity account.
        /// 1. `[signer]`   Leader of the node.
        /// 2. `[]`         Rent sysvar.
        CreateEntity {
            /// The Serum ecosystem duties a Node performs to earn extra performance
            /// based rewards, for example, cranking.
            capabilities: u32,
            /// Type of governance backing the `Entity`. For simplicity in the first version,
            /// all `nodes` will be `delegated-staked`, which means the `node-leader`
            /// will execute governance decisions.
            stake_kind: crate::accounts::StakeKind,
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
        /// 3. `[]`         Rent sysvar.
        JoinEntity {
            /// The owner of this entity account. Must sign off when staking and
            /// withdrawing.
            beneficiary: Pubkey,
            /// An account that can withdrawal or stake on the beneficiary's
            /// behalf.
            delegate: Pubkey,
        },
        // TODO: update member to change delegate access.
        /// Deposits funds into the staking pool on behalf Member account of
        /// the Member account, issuing staking pool tokens as proof of deposit.
        ///
        /// Fails if there is less than 1 MSRM in the associated `Entity`
        /// account *or* the deposit is less than 1 MSRM.
        ///
        /// Accounts:
        ///
        /// 0. `[signer]`   Owner of the depositing token account.
        /// 1. `[]`         The depositing token account.
        /// 2. `[writable]` Member account responsibile for the stake.
        /// 3. `[signer]`   Beneficiary *or* delegate of the Member account
        ///                 being staked.
        /// 4. `[writable]` Entity account to stake to.
        /// 5. `[]`         SPL token program.
        #[cfg_attr(feature = "client", create_account(*member::SIZE))]
        Stake {
            // Amount of of the token to stake with the entity.
            amount: u64,
            // True iff staking MSRM.
            is_mega: bool,
        },
        /// Initiates a stake withdrawal. Funds are locked up until the
        /// withdrawl timelock passes.
        ///
        /// Accounts:
        ///
        /// 0. `[writable]  PendingWithdrawal account to initialize.
        /// 0  `[signed]`   Benficiary/delegate of the Stake account.
        /// 1. `[writable]` The Member account to withdraw from.
        /// 2. `[writable]` Entity the Stake is associated with.
        /// 3. `[signed]`   Owner of the staking pool token account to redeem.
        StartStakeWithdrawal { amount: u64, mega_amount: u64 },
        /// Completes the pending withdrawal once the timelock period passes.
        ///
        /// Accounts:
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

#[cfg(feature = "client")]
pub mod client_ext;
#[cfg(feature = "client")]
pub use client_ext::client;
#[cfg(feature = "client")]
pub use client_ext::instruction;

pub mod accounts;
pub mod error;

serum_common::packable!(crate::instruction::RegistryInstruction);
