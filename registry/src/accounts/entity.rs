use crate::accounts::Registrar;
use crate::error::RegistryError;
use borsh::{BorshDeserialize, BorshSchema, BorshSerialize};
use serum_common::pack::*;
use solana_client_gen::solana_sdk::pubkey::Pubkey;
use solana_client_gen::solana_sdk::sysvar::clock::Clock;

#[cfg(feature = "client")]
lazy_static::lazy_static! {
    pub static ref SIZE: u64 = Entity::default()
                .size()
                .expect("Entity has a fixed size");
}

// Conversion from base unit of SRM to base unit of MSRM.
//
// SRM has 6 decimals. MSRM ~ 1_000_000 SRM => 1 unit of
// the MSRM mint == 10**6 * 1_000_000 units of the SRM mint.
const MSRM_SRM_RATE: u64 = 1_000_000_000_000;

#[derive(Clone, Debug, BorshSerialize, BorshDeserialize, BorshSchema)]
pub struct Entity {
    /// Set when this entity is registered.
    pub initialized: bool,
    /// The registrar to which this Member belongs.
    pub registrar: Pubkey,
    /// Leader of the entity.
    pub leader: Pubkey,
    /// Cumulative balances from all member accounts.
    pub balances: Balances,
    /// See `EntityState` comments.
    pub state: EntityState,
    /// Arbitrary metadata account owned by any program.
    pub metadata: Pubkey,
}

impl Default for Entity {
    fn default() -> Entity {
        Entity {
            initialized: false,
            registrar: Pubkey::new_from_array([0; 32]),
            leader: Pubkey::new_from_array([0; 32]),
            balances: Balances::default(),
            state: EntityState::PendingDeactivation {
                deactivation_start_ts: 0,
                timelock: 0,
            },
            metadata: Pubkey::new_from_array([0; 32]),
        }
    }
}

impl Entity {
    pub fn spt_did_stake(&mut self, amount: u64, is_mega: bool) -> Result<(), RegistryError> {
        if is_mega {
            self.balances.spt_mega_amount += amount;
        } else {
            self.balances.spt_amount += amount;
        }

        Ok(())
    }

    pub fn spt_did_unstake_start(&mut self, spt_amount: u64, mega: bool) {
        if mega {
            self.balances.spt_mega_amount -= spt_amount;
        } else {
            self.balances.spt_amount -= spt_amount;
        }
    }

    #[inline(never)]
    pub fn transition_activation_if_needed(&mut self, registrar: &Registrar, clock: &Clock) {
        match self.state {
            EntityState::Inactive => {
                if self.meets_activation_requirements() {
                    self.state = EntityState::Active;
                }
            }
            EntityState::PendingDeactivation {
                deactivation_start_ts,
                timelock,
            } => {
                if clock.unix_timestamp > deactivation_start_ts + timelock {
                    self.state = EntityState::Inactive;
                }
                if self.meets_activation_requirements() {
                    self.state = EntityState::Active;
                }
            }
            EntityState::Active => {
                if !self.meets_activation_requirements() {
                    self.state = EntityState::PendingDeactivation {
                        deactivation_start_ts: clock.unix_timestamp,
                        timelock: registrar.deactivation_timelock,
                    }
                }
            }
        }
    }

    pub fn meets_activation_requirements(&self) -> bool {
        self.balances.spt_mega_amount >= 1
    }

    pub fn stake_will_max(&self, spt_amount: u64, is_mega: bool, registrar: &Registrar) -> bool {
        let spt_value = {
            if is_mega {
                spt_amount
                    .checked_mul(registrar.stake_rate_mega)
                    .unwrap()
                    .checked_mul(MSRM_SRM_RATE)
                    .unwrap()
            } else {
                spt_amount.checked_mul(registrar.stake_rate).unwrap()
            }
        };
        let amount_equivalent = spt_value
            .checked_add(
                self.balances
                    .spt_amount
                    .checked_mul(registrar.stake_rate)
                    .unwrap(),
            )
            .unwrap()
            .checked_add(
                self.balances
                    .spt_mega_amount
                    .checked_mul(registrar.stake_rate_mega)
                    .unwrap()
                    .checked_mul(MSRM_SRM_RATE)
                    .unwrap(),
            )
            .unwrap();
        amount_equivalent > registrar.max_stake_per_entity
    }
}

serum_common::packable!(Entity);

#[derive(Clone, Default, Debug, BorshSerialize, BorshDeserialize, BorshSchema)]
pub struct Balances {
    // Denominated in staking pool tokens.
    pub spt_amount: u64,
    pub spt_mega_amount: u64,
}

#[derive(Clone, Debug, BorshSerialize, BorshDeserialize, BorshSchema, PartialEq)]
pub enum EntityState {
    /// The entity is ineligble for rewards. Redeeming existing staking pool
    /// tokens will return less than or equal to the original staking deposit.
    Inactive,
    /// The Entity is on a deactivation countdown, lasting until the timestamp
    /// Registrar.deactivation_timelock`, at which point the EntityState
    /// transitions from PendingDeactivation to Inactive.
    ///
    /// During this time, either members  must stake more SRM or MSRM or they
    /// should withdraw their stake to retrieve their rewards.
    PendingDeactivation {
        deactivation_start_ts: i64,
        timelock: i64,
    },
    /// The entity is eligble for rewards. Member accounts can stake with this
    /// entity and receive rewards.
    Active,
}

impl Default for EntityState {
    fn default() -> Self {
        Self::Inactive
    }
}
