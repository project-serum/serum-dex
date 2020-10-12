use crate::access_control;
use crate::accounts::Registrar;
use crate::error::RegistryError;
use borsh::{BorshDeserialize, BorshSchema, BorshSerialize};
use num_enum::IntoPrimitive;
use serum_common::pack::*;
use solana_client_gen::solana_sdk::account_info::AccountInfo;
use solana_client_gen::solana_sdk::pubkey::Pubkey;
use solana_client_gen::solana_sdk::sysvar::clock::Clock;
use std::convert::Into;

#[cfg(feature = "client")]
lazy_static::lazy_static! {
    pub static ref SIZE: u64 = Entity::default()
                .size()
                .expect("Entity has a fixed size");
}

/// Entity is the account representing a single "node" that addresses can
/// stake with.
#[derive(Default, Debug, BorshSerialize, BorshDeserialize, BorshSchema)]
pub struct Entity {
    /// Set when this entity is registered with the program.
    pub initialized: bool,
    /// The registrar to which this Member belongs.
    pub registrar: Pubkey,
    /// Leader of the entity, i.e., the one responsible for fulfilling node
    /// duties.
    pub leader: Pubkey,
    /// Bitmap representing this entity's capabilities .
    pub capabilities: u32,
    /// Type of stake backing this entity (determines voting rights)
    /// of the stakers.
    pub stake_kind: StakeKind,
    /// Cumulative stake balances from all member accounts.
    pub balances: Balances,
    /// The activation generation number, incremented whenever EntityState
    /// transitions froom `Inactive` -> `Active`.
    pub generation: u64,
    /// State of the Entity. See the `EntityState` comments.
    pub state: EntityState,
}

// Public methods.
impl Entity {
    /// Returns the amount of stake contributing to the activation level.
    pub fn activation_amount(&self) -> u64 {
        self.amount_equivalent() + self.stake_intent_equivalent()
    }

    /// Adds to the stake intent balance.
    pub fn add_stake_intent(
        &mut self,
        amount: u64,
        mega: bool,
        registrar: &Registrar,
        clock: &Clock,
    ) {
        if mega {
            self.balances.mega_stake_intent += amount;
        } else {
            self.balances.stake_intent += amount;
        }
        self.transition_activation_if_needed(registrar, clock);
    }

    /// Subtracts from the stake intent balance.
    pub fn sub_stake_intent(
        &mut self,
        amount: u64,
        mega: bool,
        registrar: &Registrar,
        clock: &Clock,
    ) {
        if mega {
            self.balances.mega_stake_intent -= amount;
        } else {
            self.balances.stake_intent -= amount;
        }
        self.transition_activation_if_needed(registrar, clock);
    }

    /// Adds to the stake balance.
    pub fn add_stake(&mut self, amount: u64, is_mega: bool, registrar: &Registrar, clock: &Clock) {
        if is_mega {
            self.balances.mega_stake_intent += amount;
        } else {
            self.balances.stake_intent += amount;
        }
        self.transition_activation_if_needed(registrar, clock);
    }

    /// Moves stake into the pending wtihdrawal state.
    pub fn transfer_pending_withdrawal(
        &mut self,
        amount: u64,
        mega: bool,
        registrar: &Registrar,
        clock: &Clock,
    ) {
        if mega {
            self.balances.mega_amount -= amount;
            self.balances.mega_pending_withdrawals += amount;
        } else {
            self.balances.amount -= amount;
            self.balances.pending_withdrawals += amount;
        }
        self.transition_activation_if_needed(registrar, clock);
    }

    /// Transitions the EntityState finite state machine. This should be called
    /// immediately before processing any instruction relying on the most up
    /// to date status of the EntityState. It should also be called after any
    /// mutation to the SRM equivalent deposit of this entity to keep the state
    /// up to date.
    pub fn transition_activation_if_needed(&mut self, registrar: &Registrar, clock: &Clock) {
        match self.state {
            EntityState::Inactive => {
                if self.meets_activation_requirements(registrar) {
                    self.state = EntityState::Active;
                    self.generation += 1;
                }
            }
            EntityState::PendingDeactivation {
                deactivation_start_ts,
            } => {
                if clock.unix_timestamp > deactivation_start_ts + registrar.deactivation_timelock()
                {
                    self.state = EntityState::Inactive;
                }
            }
            EntityState::Active => {
                if !self.meets_activation_requirements(registrar) {
                    self.state = EntityState::PendingDeactivation {
                        deactivation_start_ts: clock.unix_timestamp,
                    }
                }
            }
        }
    }

    /// Returns true if this Entity is capable of being "activated", i.e., can
    /// enter the staking pool.
    pub fn meets_activation_requirements(&self, registrar: &Registrar) -> bool {
        self.activation_amount() >= registrar.reward_activation_threshold
            && self.balances.mega_amount >= 1
    }
}

// Private methods.
impl Entity {
    fn amount_equivalent(&self) -> u64 {
        self.balances.amount + self.balances.mega_amount * 1_000_000
    }
    fn stake_intent_equivalent(&self) -> u64 {
        self.balances.stake_intent + self.balances.mega_stake_intent * 1_000_000
    }
}

serum_common::packable!(Entity);

#[derive(Default, Debug, BorshSerialize, BorshDeserialize, BorshSchema)]
pub struct Balances {
    pub amount: u64,
    pub mega_amount: u64,
    pub stake_intent: u64,
    pub mega_stake_intent: u64,
    pub pending_withdrawals: u64,
    pub mega_pending_withdrawals: u64,
}

/// EntityState defines a finite-state-machine (FSM) determining the actions
/// a `Member` account can take with respect to staking an Entity and receiving
/// rewards.
///
/// FSM:
///
/// Inactive -> Active:
///  * Entity `generation` count gets incremented and Members may stake.
/// Active -> PendingDeactivation:
///  * Staking ceases and Member accounts should withdraw or add more
///    stake-intent.
/// PendingDeactivation -> Active:
///  * New stake is accepted and rewards continue.
/// PendingDeactivation -> Inactive:
///  * Stake not withdrawn will not receive accrued rewards (just original
///    deposit). If the Entity becomes active again, Members with deposits
///    from old "generations" must withdraw their entire deposit, before being
///    allowed to stake again.
///
#[derive(Debug, BorshSerialize, BorshDeserialize, BorshSchema, PartialEq)]
pub enum EntityState {
    /// The entity is ineligble for rewards. Redeeming existing staking pool
    /// tokens will return less than or equal to the original staking deposit.
    Inactive,
    /// The Entity is on a deactivation countdown, lasting until the timestamp
    /// `deactivation_start_ts + Registrar.deactivation_timelock_premium`,
    /// at which point the EntityState transitions from PendingDeactivation
    /// to Inactive.
    ///
    /// During this time, either members  must stake more SRM or MSRM or they
    /// should withdraw their stake to retrieve their rewards.
    PendingDeactivation { deactivation_start_ts: i64 },
    /// The entity is eligble for rewards. Member accounts can stake with this
    /// entity and receive rewards.
    Active,
}

impl Default for EntityState {
    fn default() -> Self {
        Self::Inactive
    }
}

#[derive(
    Debug, PartialEq, IntoPrimitive, Clone, Copy, BorshSerialize, BorshDeserialize, BorshSchema,
)]
#[repr(u32)]
pub enum StakeKind {
    Voting,
    Delegated,
}

impl Default for StakeKind {
    fn default() -> Self {
        StakeKind::Delegated
    }
}

// with_entity should be used for any instruction relying on the most up to
// date `state` of an Entity.
//
//
// As time, passes, it's possible an Entity's internal FSM *should* have
// transitioned (i.e., from PendingDeactivation -> Inactive), but didn't
// because no transaction was invoked.
//
// This method transitions the Entity's state, before performing the action
// provided by the given closure.
pub fn with_entity<F>(req: WithEntityRequest, f: &mut F) -> Result<(), RegistryError>
where
    F: FnMut(&mut Entity, &Registrar, &Clock) -> Result<(), RegistryError>,
{
    let WithEntityRequest {
        entity,
        registrar,
        clock,
        program_id,
    } = req;
    Entity::unpack_mut(
        &mut entity.try_borrow_mut_data()?,
        &mut |entity: &mut Entity| {
            let clock = access_control::clock(&clock)?;
            let registrar = access_control::registrar(&registrar, program_id)?;
            entity.transition_activation_if_needed(&registrar, &clock);

            f(entity, &registrar, &clock).map_err(Into::into)
        },
    )?;
    Ok(())
}

pub struct WithEntityRequest<'a> {
    pub entity: &'a AccountInfo<'a>,
    pub registrar: &'a AccountInfo<'a>,
    pub clock: &'a AccountInfo<'a>,
    pub program_id: &'a Pubkey,
}
