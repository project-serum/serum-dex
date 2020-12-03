use crate::accounts::{Member, Registrar};
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
    pub fn remove(&mut self, member: &mut Member) {
        self.balances.current_deposit -= member.balances.current_deposit;
        self.balances.current_mega_deposit -= member.balances.current_mega_deposit;
        self.balances.spt_amount -= member.balances.spt_amount;
        self.balances.spt_mega_amount -= member.balances.spt_mega_amount;
    }

    pub fn add(&mut self, member: &mut Member) {
        self.balances.current_deposit += member.balances.current_deposit;
        self.balances.current_mega_deposit += member.balances.current_mega_deposit;
        self.balances.spt_amount += member.balances.spt_amount;
        self.balances.spt_mega_amount += member.balances.spt_mega_amount;
    }

    pub fn activation_amount(&self) -> u64 {
        self.amount_equivalent() + self.current_deposit_equivalent()
    }

    pub fn did_deposit(&mut self, amount: u64, mega: bool) {
        if mega {
            self.balances.current_mega_deposit += amount;
        } else {
            self.balances.current_deposit += amount;
        }
    }

    pub fn did_withdraw(&mut self, amount: u64, mega: bool) {
        if mega {
            self.balances.current_mega_deposit -= amount;
        } else {
            self.balances.current_deposit -= amount;
        }
    }

    pub fn spt_did_stake(&mut self, amount: u64, is_mega: bool) -> Result<(), RegistryError> {
        if is_mega {
            self.balances.spt_mega_amount += amount;
            self.balances.current_mega_deposit -= amount;
        } else {
            self.balances.spt_amount += amount;
            self.balances.current_deposit -= amount;
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

    pub fn spt_did_unstake_end(&mut self, amount: u64, is_mega: bool) {
        if is_mega {
            self.balances.current_mega_deposit += amount;
        } else {
            self.balances.current_deposit += amount;
        }
    }

    #[inline(never)]
    pub fn transition_activation_if_needed(&mut self, registrar: &Registrar, clock: &Clock) {
        match self.state {
            EntityState::Inactive => {
                if self.meets_activation_requirements(registrar) {
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
                if self.meets_activation_requirements(registrar) {
                    self.state = EntityState::Active;
                }
            }
            EntityState::Active => {
                if !self.meets_activation_requirements(registrar) {
                    self.state = EntityState::PendingDeactivation {
                        deactivation_start_ts: clock.unix_timestamp,
                        timelock: registrar.deactivation_timelock,
                    }
                }
            }
        }
    }

    /// Returns true if this Entity is capable of being "activated", i.e., can
    /// enter the staking pool.
    pub fn meets_activation_requirements(&self, registrar: &Registrar) -> bool {
        self.activation_amount() >= registrar.reward_activation_threshold
            && (self.balances.spt_mega_amount >= 1 || self.balances.current_mega_deposit >= 1)
    }

    pub fn slash(&mut self, spt_amount: u64, mega: bool) {
        if mega {
            self.balances.spt_mega_amount -= spt_amount;
        } else {
            self.balances.spt_amount -= spt_amount;
        }
    }

    pub fn amount_equivalent(&self) -> u64 {
        self.balances.spt_amount + self.balances.spt_mega_amount * 1_000_000
    }

    fn current_deposit_equivalent(&self) -> u64 {
        self.balances.current_deposit + self.balances.current_mega_deposit * 1_000_000
    }
}

serum_common::packable!(Entity);

#[derive(Clone, Default, Debug, BorshSerialize, BorshDeserialize, BorshSchema)]
pub struct Balances {
    // Denominated in staking pool tokens.
    pub spt_amount: u64,
    pub spt_mega_amount: u64,
    // Denominated in SRM/MSRM.
    pub current_deposit: u64,
    pub current_mega_deposit: u64,
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
