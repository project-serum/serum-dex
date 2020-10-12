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
    pub enum RewardsInstruction {
        /// Accounts:
        ///
        /// 0. `[writable]` Rewards instance to initialize.
        /// 1. `[]`         Vault.
        /// 2. `[]`         Registrar.
        /// 3. `[]`         Rent sysvar.
        Initialize {
            nonce: u8,
            registry_program_id: Pubkey,
            dex_program_id: Pubkey,
            authority: Pubkey,
        },
        /// CrankRelay proxies a `ConsumeEvents` instruction to the configured
        /// dex and pays out a reward as a function of the number of events
        /// in the queue cranked and the fee rate in the node Registry.
        ///
        /// Accounts:
        ///
        /// 0. `[]`         Instance.
        /// 1. `[writable]` Vault.
        /// 2. `[]`         Registrar.
        /// 3. `[writable]` Receiving token account.
        /// 4. `[]`         Entity.
        /// 5. `[signed]`   Entity leader.
        /// 6. `[]`         Token program.
        /// 7. `[]`         DEX (relay) program.
        /// 8. `[writable]` Event queue.
        ///
        /// Relay:
        ///
        /// .. `[]`         Program specific relay accounts.
        CrankRelay { instruction_data: Vec<u8> },
        /// Accounts:
        ///
        /// 0. `[signed]`   Instance authority.
        /// 1. `[writable]` Instance.
        SetAuthority { authority: Pubkey },
        /// Moves funds to the new address.
        ///
        /// Accounts:
        ///
        /// 0. `[signer]`   Instance authority.
        /// 1. `[writable]` Instance account.
        /// 2. `[writable]` Instance vault.
        /// 3. `[]`         Instance vault authority.
        /// 4. `[writable]` Migrated token account.
        /// 5. `[]`         SPL token program.
        Migrate,
    }
}

serum_common::packable!(instruction::RewardsInstruction);
