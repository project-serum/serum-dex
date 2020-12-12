#![cfg_attr(feature = "strict", deny(warnings))]
#![allow(dead_code)]

use borsh::{BorshDeserialize, BorshSerialize};
use serum_common::pack::*;
use solana_client_gen::prelude::*;

pub mod access_control;
pub mod accounts;
pub mod error;

#[cfg_attr(feature = "client", solana_client_gen)]
pub mod instruction {
    use super::*;
    #[derive(Debug, BorshSerialize, BorshDeserialize)]
    pub enum RegistryInstruction {
        /// Accounts:
        ///
        /// 0. `[writable]` Registrar.
        /// 1. `[]`         Vault.
        /// 2. `[]`         Mega vault.
        /// 3. `[]`         Pool.
        /// 4. `[]`         Mega pool.
        /// 5. `[]`         Pool program.
        /// 6. `[]`         Rent sysvar.
        Initialize {
            authority: Pubkey,
            mint: Pubkey,
            mint_mega: Pubkey,
            nonce: u8,
            withdrawal_timelock: i64,
            deactivation_timelock: i64,
            reward_activation_threshold: u64,
            max_stake_per_entity: u64,
            stake_rate: u64,
            stake_rate_mega: u64,
        },
        /// Accounts:
        ///
        /// 0. `[writable]` Registrar.
        /// 1. `[signer]`   Authority.
        UpdateRegistrar {
            new_authority: Option<Pubkey>,
            withdrawal_timelock: Option<i64>,
            deactivation_timelock: Option<i64>,
            reward_activation_threshold: Option<u64>,
            max_stake_per_entity: Option<u64>,
        },
        /// Accounts:
        ///
        /// 0. `[writable]` Entity.
        /// 1. `[signer]`   Entity leader.
        /// 2. `[]`         Registrar.
        /// 3. `[]`         Rent sysvar.
        CreateEntity { metadata: Pubkey },
        /// Accounts:
        ///
        /// 0. `[writable]` Entity account.
        /// 1. `[signer]`   Entity leader.
        /// 2. `[]`         Registrar.
        UpdateEntity {
            leader: Option<Pubkey>,
            metadata: Option<Pubkey>,
        },
        /// Accounts:
        ///
        /// 0. `[signer]`   Beneficiary.
        /// 1. `[writable]` Member.
        /// 2. `[]`         Entity to join.
        /// 3. `[]`         Registrar.
        /// 2. `[]`         Staking pool token.
        /// 3. `[]`         Mega staking pool token.
        /// 4. `[]`         Rent sysvar.
        CreateMember,
        /// Accounts:
        ///
        /// 0. `[writable]` Member.
        /// 1. `[signer]`   Beneficiary.
        UpdateMember { metadata: Option<Pubkey> },
        /// Accounts:
        ///
        /// 0. `[writable]` Member.
        /// 1. `[signed]`   Beneficiary.
        /// 2. `[]`         Registrar.
        /// 3. `[writable]` Current entity.
        /// 4. `[writable]` New entity.
        /// 5. `[]`         Clock sysvar.
        ///
        /// ..              GetBasket pool accounts.
        SwitchEntity,
        /// Accounts:
        ///
        /// Lockup whitelist relay interface (funds flow *from* lockup program):
        ///
        /// 0. `[writable]`  Depositor token account.
        /// 1. `[]`          Depositor token authority.
        /// 2. `[]`          Token program.
        ///
        /// Program specific.
        ///
        /// 3. `[writable]` Member.
        /// 4. `[signer]`   Beneficiary.
        /// 5. `[writable]` Entity.
        /// 6. `[]`         Registrar.
        /// 7. `[]`         Clock.
        /// 8. `[]`         Vault (either the MSRM or SRM vault depending on
        ///                 depositor's mint).
        Deposit { amount: u64 },
        /// Accounts:
        ///
        /// Lockup whitelist relay interface (funds flow *to* lockup program):
        ///
        /// 0. `[writable]`  Depositor token account.
        /// 1. `[]`          Depositor token authority.
        /// 2. `[]`          Token program.
        /// 3. `[]`          Vault authority.
        ///
        /// Program specific.
        ///
        /// 4. `[writable]` Member.
        /// 5. `[signer]`   Beneficiary.
        /// 6. `[writable]` Entity.
        /// 7. `[]`         Registrar.
        /// 8. `[]`         Clock.
        /// 9. `[]`         Vault (either the MSRM or SRM vault depending on
        ///                 depositor's mint).
        Withdraw { amount: u64 },
        /// Accounts:
        ///
        /// 0. `[writable]` Member.
        /// 1. `[signer]`   Beneficiary.
        /// 2. `[writable]` Entity.
        /// 3. `[]`         Registrar.
        /// 4. `[]`         Clock sysvar.
        /// 5. `[]`         Token program.
        Stake { amount: u64, balance_id: Pubkey },
        /// Accounts:
        ///
        /// 0. `[writable]  PendingWithdrawal.
        /// 1. `[writable]` Member.
        /// 2  `[signed]`   Benficiary.
        /// 3. `[writable]` Entity.
        /// 4. `[writable]` Registrar.
        /// 5. `[]`         Vault authority.
        /// 7. `[]`         Token program.
        /// 8. `[]`         Clock sysvar.
        /// 9. `[]`         Rent sysvar.
        StartStakeWithdrawal { amount: u64, balance_id: Pubkey },
        /// Accounts:
        ///
        /// 0. `[writable]  PendingWithdrawal.
        /// 1. `[writable]` Member.
        /// 2. `[signed]`   Beneficiary.
        /// 3. `[writable]` Entity.
        /// 4. `[]`         Registrar.
        /// 5. `[]`         Clock.
        EndStakeWithdrawal,
        /// Accounts: TODO
        ///
        ///
        DropLockedReward {
            total: u64,
            end_ts: i64,
            expiry_ts: i64,
            expiry_receiver: Pubkey,
            period_count: u64,
            nonce: u8,
        },
        /// Accounts: TODO
        ///
        ///
        DropUnlockedReward {
            total: u64,
            expiry_ts: i64,
            expiry_receiver: Pubkey,
            nonce: u8,
        },
        /// Accounts: TODO
        ///
        ///
        ClaimLockedReward { cursor: u32, nonce: u8 },
        /// Accounts: TODO
        ///
        ///
        ClaimUnlockedReward { cursor: u32 },
        /// Accounts:
        ///
        /// 0. `[signer]`   Expiry receiver.
        /// 1. `[writable]` Token account to send leftover rewards to.
        /// 2. `[writable]` Vendor.
        /// 3. `[writable]` Vendor vault.
        /// 4. `[]`         Vendor vault authority.
        /// 5. `[]`         Registrar.
        /// 6. `[]`         Token program.
        /// 7. `[]`         Clock sysvar.
        ExpireUnlockedReward,
        /// Same as ExpireUnlockedReward, but with a LockedRewardVendor
        /// account.
        ExpireLockedReward,
    }
}

serum_common::packable!(instruction::RegistryInstruction);
