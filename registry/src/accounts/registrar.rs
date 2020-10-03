use serum_common::pack::*;
use solana_client_gen::solana_sdk::pubkey::Pubkey;

#[cfg(feature = "client")]
lazy_static::lazy_static! {
    pub static ref SIZE: u64 = Registrar::default()
                .size()
                .expect("Vesting has a fixed size");
}

/// Registry defines the account representing an instance of the program.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Registrar {
    /// Set by the program on initialization.
    pub initialized: bool,
    /// Priviledged account with the ability to register capabilities.
    pub authority: Pubkey,
    /// Maps capability identifier to the bps fee rate earned for the capability.
    pub capabilities_fees_bps: [u32; 32],
    /// Number of slots that must pass for a withdrawal to complete.
    pub withdrawal_timelock: u64,
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
}

serum_common::packable!(Registrar);
