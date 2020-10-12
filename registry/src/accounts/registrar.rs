use borsh::{BorshDeserialize, BorshSchema, BorshSerialize};
use serum_common::pack::*;
use solana_client_gen::solana_sdk::pubkey::Pubkey;

#[cfg(feature = "client")]
lazy_static::lazy_static! {
    pub static ref SIZE: u64 = Registrar::default()
                .size()
                .expect("Vesting has a fixed size");
}

pub const CAPABILITY_LEN: u8 = 32;

/// Registry defines the account representing an instance of the program.
#[derive(Clone, Debug, Default, PartialEq, BorshSerialize, BorshDeserialize, BorshSchema)]
pub struct Registrar {
    /// Set by the program on initialization.
    pub initialized: bool,
    /// Priviledged account with the ability to register capabilities.
    pub authority: Pubkey,
    /// Nonce to derive the program-derived address owning the vaults.
    pub nonce: u8,
    /// Maps capability identifier to the bps fee rate earned for the capability.
    pub capabilities_fees_bps: [u32; CAPABILITY_LEN as usize],
    /// Address of the capabilities list account, in the event we want to
    /// enforce access control on capabilities addresses.
    pub capabilities_list: Pubkey,
    /// Number of seconds that must pass for a withdrawal to complete.
    pub withdrawal_timelock: i64,
    /// Number of seconds *in addition* to the withdrawal timelock it takes for
    /// an Entity account to be "deactivated"--i.e., cant receive rewards--from
    /// the moment it's SRM equivalent staked amount drops below the required
    /// threshold.
    pub deactivation_timelock_premium: i64,
    /// Vault holding stake-intent tokens.
    pub vault: Pubkey,
    /// Vault holding stake-intent mega tokens.
    pub mega_vault: Pubkey,
    /// The amount of tokens that must be deposited to be eligible for rewards.
    pub reward_activation_threshold: u64,
}

impl Registrar {
    /// Returns the capability id of the next available slot. Otherwise None,
    /// if full.
    pub fn next_free_capability_id(&self) -> Option<u8> {
        for (idx, c) in self.capabilities_fees_bps.iter().enumerate() {
            if *c == 0 {
                return Some(idx as u8);
            }
        }
        None
    }

    pub fn deactivation_timelock(&self) -> i64 {
        self.deactivation_timelock_premium + self.withdrawal_timelock
    }

    // Assumes capability_id <= CAPABILITY_SIZE.
    pub fn fee_rate(&self, capability_id: usize) -> u32 {
        self.capabilities_fees_bps[capability_id]
    }
}

serum_common::packable!(Registrar);
