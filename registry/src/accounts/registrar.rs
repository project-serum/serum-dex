use crate::error::{RegistryError, RegistryErrorCode};
use borsh::{BorshDeserialize, BorshSchema, BorshSerialize};
use serum_common::pack::*;
use solana_client_gen::solana_sdk::pubkey::Pubkey;

#[cfg(feature = "client")]
lazy_static::lazy_static! {
    pub static ref SIZE: u64 = Registrar::default()
                .size()
                .expect("Registrar has a fixed size");
}

/// Registry defines the account representing an instance of the program.
#[derive(Clone, Debug, Default, PartialEq, BorshSerialize, BorshDeserialize, BorshSchema)]
pub struct Registrar {
    /// Set by the program on initialization.
    pub initialized: bool,
    /// Priviledged account.
    pub authority: Pubkey,
    /// Nonce to derive the program-derived address owning the vaults.
    pub nonce: u8,
    /// The amount of tokens that must be deposited to be eligible for rewards,
    /// denominated in SRM.
    pub reward_activation_threshold: u64,
    /// The maximum stake per node entity, denominated in SRM.
    pub max_stake_per_entity: u64,
    /// Number of seconds that must pass for a withdrawal to complete.
    pub withdrawal_timelock: i64,
    /// Number of seconds it takes for an Entity to be "deactivated", from the
    /// moment it's SRM/MSRM amount drops below the required threshold.
    pub deactivation_timelock: i64,
    ///
    pub reward_event_q: Pubkey,
    /// Vault holding deposit tokens.
    pub vault: Pubkey,
    /// Vault holding deposit mega tokens.
    pub mega_vault: Pubkey,
    /// Address of the SRM staking pool.
    pub pool_vault: Pubkey,
    /// Address of the MSRM staking pool.
    pub pool_vault_mega: Pubkey,
    ///
    pub pool_mint: Pubkey,
    ///
    pub pool_mint_mega: Pubkey,
}

impl Registrar {
    pub fn is_mega(&self, key: Pubkey) -> Result<bool, RegistryError> {
        if key == self.vault {
            Ok(false)
        } else if key == self.mega_vault {
            Ok(true)
        } else {
            Err(RegistryErrorCode::InvalidVault.into())
        }
    }
}

serum_common::packable!(Registrar);
