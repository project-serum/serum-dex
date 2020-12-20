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
        /// 1. `[]`         Pool mint.
        /// 2. `[]`         Mega pool mint.
        /// 3. `[]`         Reward event q.
        /// 4. `[]`         Rent sysvar.
        Initialize {
            authority: Pubkey,
            mint: Pubkey,
            mint_mega: Pubkey,
            nonce: u8,
            withdrawal_timelock: i64,
            deactivation_timelock: i64,
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
            max_stake_per_entity: Option<u64>,
        },
        /// Accounts:
        ///
        /// 0. `[writable]` Entity.
        /// 1. `[signer]`   Leader.
        /// 2. `[]`         Registrar.
        /// 3. `[]`         Rent sysvar.
        CreateEntity {
            metadata: Pubkey,
        },
        /// Accounts:
        ///
        /// 0. `[writable]` Entity account.
        /// 1. `[signer]`   Leader.
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
        /// 4. `[]`         Registrar signer/vault authority.
        /// 5. `[]`         Token program.
        /// 6. `[]`         Rent sysvar.
        /// ..              Balance sandboxes.
        CreateMember,
        /// Accounts:
        ///
        /// 0. `[writable]` Member.
        /// 1. `[signer]`   Beneficiary.
        UpdateMember {
            metadata: Option<Pubkey>,
        },
        /// Accounts:
        ///
        /// 0. `[writable]` Member.
        /// 1. `[signed]`   Beneficiary.
        /// 2. `[]`         Registrar.
        /// 3. `[writable]` Current entity.
        /// 4. `[writable]` New entity.
        /// 5. `[]`         Clock sysvar.
        /// 6. `[]`         Vault authority.
        /// 7. `[]`         Reward q.
        /// .. `[]`         Stake assets.
        SwitchEntity,
        /// Accounts:
        ///
        /// Lockup whitelist relay interface (funds can flow *from* lockup program):
        ///
        /// 0. `[]`          Vesting account (unused dummy account).
        /// 1. `[writable]`  Depositor token account.
        /// 2. `[signer]`    Depositor token authority.
        /// 3. `[]`          Token program.
        /// 4. `[writable]`  Member vault.
        /// 5. `[]`          Member vault authority.
        ///
        /// Program specific.
        ///
        /// 6. `[writable]` Member.
        /// 7. `[signer]`   Beneficiary.
        /// 8. `[writable]` Entity.
        /// 9. `[]`         Registrar.
        Deposit {
            amount: u64,
        },
        /// Accounts:
        ///
        /// Lockup whitelist relay interface (funds can flow *to* lockup program):
        ///
        /// 0. `[]`          Vesting account (unused dummy account).
        /// 1. `[writable]`  Depositor token account.
        /// 2. `[signer]`    Depositor token authority.
        /// 3. `[]`          Token program.
        /// 4. `[writable]`  Member vault.
        /// 5. `[]`          Member vault authority.
        ///
        /// Program specific.
        ///
        /// 6. `[writable]` Member.
        /// 7. `[signer]`   Beneficiary.
        /// 8. `[writable]` Entity.
        /// 9. `[]`         Registrar.
        Withdraw {
            amount: u64,
        },
        /// Accounts:
        ///
        /// 0. `[writable]` Member.
        /// 1. `[signer]`   Beneficiary.
        /// 2. `[writable]` Entity.
        /// 3. `[]`         Registrar.
        /// 4. `[writable]` Deposit vault.
        /// 5. `[]`         Vault authority.
        /// 6. `[writable]` Stake vault.
        /// 7. `[writable]` Stake pool token mint.
        /// 8. `[writable]` Stake pool token.
        /// 9. `[]`         Reward q.
        /// 10. `[]`        Clock sysvar.
        /// 11. `[]`        Token program.
        /// ..  `[]         Stake assets.
        Stake {
            amount: u64,
            balance_id: Pubkey,
        },
        /// Accounts:
        ///
        /// 0. `[writable]  PendingWithdrawal.
        /// 1. `[]`         Member.
        /// 2  `[signed]`   Benficiary.
        /// 3. `[writable]` Entity.
        /// 4. `[]`         Registrar.
        /// 5. `[writable]` Pending vault.
        /// 6. `[]`         Vault authority.
        /// 7. `[writable]` Stake vault.
        /// 8. `[writable]` Stake pool token mint.
        /// 9. `[writable]` Stake pool token.
        /// 10. `[]`        Token program.
        /// 11. `[]`        Rent sysvar.
        /// 12. `[]`        Reward q.
        /// ..  `[]`        Stake assets.
        StartStakeWithdrawal {
            amount: u64,
            balance_id: Pubkey,
        },
        /// Accounts:
        ///
        /// 0. `[writable]  PendingWithdrawal.
        /// 1. `[writable]` Member.
        /// 2. `[writable]` Deposit vault.
        /// 3. `[writable]` Pending vault.
        /// 4. `[]`         Vault authority.
        /// 5. `[signed]`   Beneficiary.
        /// 6. `[]`         Entity.
        /// 7. `[]`         Token program.
        /// 8. `[]`         Registrar.
        /// 9. `[]`         Clock.
        EndStakeWithdrawal,
        /// Accounts:
        ///
        /// 0. `[writable]` Reward q.
        /// 1. `[]`         Registrar.
        /// 2. `[writable]` Depositing token account.
        /// 3. `[signer]`   Depositing token account owner.
        /// 4. `[]`         Pool token mint.
        /// 5. `[writable]` Locked reward vendor.
        /// 6. `[writable]` Locked reward vendor vault.
        /// 7. `[]`         Token program.
        /// 8. `[]`         Clock sysvar.
        DropLockedReward {
            total: u64,
            end_ts: i64,
            expiry_ts: i64,
            expiry_receiver: Pubkey,
            period_count: u64,
            nonce: u8,
        },
        /// Accounts:
        ///
        /// 0. `[writable]` Reward q.
        /// 1. `[]`         Registrar.
        /// 2. `[writable]` Depositing token account.
        /// 3. `[signer]`   Depositing token account owner.
        /// 4. `[]`         Pool token mint.
        /// 5. `[writable]` Unlocked reward vendor.
        /// 6. `[writable]` Unlocked reward vendor vault.
        /// 7. `[]`         Token program.
        /// 8. `[]`         Clock sysvar.
        DropUnlockedReward {
            total: u64,
            expiry_ts: i64,
            expiry_receiver: Pubkey,
            nonce: u8,
        },
        ClaimLockedReward {
            cursor: u32,
            nonce: u8,
        },
        ClaimUnlockedReward {
            cursor: u32,
        },
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
