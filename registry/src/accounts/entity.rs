use crate::accounts::{Member, Registrar};
use crate::error::{RegistryError, RegistryErrorCode};
use borsh::{BorshDeserialize, BorshSchema, BorshSerialize};
use serum_common::pack::*;
use serum_pool_schema::Basket;
use solana_client_gen::solana_sdk::pubkey::Pubkey;
use solana_client_gen::solana_sdk::sysvar::clock::Clock;
use std::convert::TryInto;

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
    /// The activation generation number, incremented whenever EntityState
    /// transitions from `Inactive` -> `Active`.
    pub generation: u64,
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
            generation: 0,
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

    pub fn activation_amount(&self, ctx: &PoolPrices) -> u64 {
        self.amount_equivalent(ctx) + self.current_deposit_equivalent()
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

    pub fn spt_did_create(
        &mut self,
        prices: &PoolPrices,
        amount: u64,
        is_mega: bool,
    ) -> Result<(), RegistryError> {
        if is_mega {
            self.balances.spt_mega_amount += amount;

            let basket = prices.basket_quantities(amount, is_mega)?;
            self.balances.current_deposit -= basket[0];
            self.balances.current_mega_deposit -= basket[1];
        } else {
            self.balances.spt_amount += amount;

            let basket = prices.basket_quantities(amount, is_mega)?;
            self.balances.current_deposit -= basket[0];
        }

        Ok(())
    }

    pub fn spt_did_redeem_start(&mut self, spt_amount: u64, mega: bool) {
        if mega {
            self.balances.spt_mega_amount -= spt_amount;
        } else {
            self.balances.spt_amount -= spt_amount;
        }
    }

    pub fn spt_did_redeem_end(&mut self, asset_amount: u64, mega_asset_amount: u64) {
        self.balances.current_deposit += asset_amount;
        self.balances.current_mega_deposit += mega_asset_amount;
    }

    #[inline(never)]
    pub fn transition_activation_if_needed(
        &mut self,
        ctx: &PoolPrices,
        registrar: &Registrar,
        clock: &Clock,
    ) {
        match self.state {
            EntityState::Inactive => {
                if self.meets_activation_requirements(ctx, registrar) {
                    self.state = EntityState::Active;
                    self.generation += 1;
                }
            }
            EntityState::PendingDeactivation {
                deactivation_start_ts,
                timelock,
            } => {
                if clock.unix_timestamp > deactivation_start_ts + timelock {
                    self.state = EntityState::Inactive;
                }
                if self.meets_activation_requirements(ctx, registrar) {
                    self.state = EntityState::Active;
                }
            }
            EntityState::Active => {
                if !self.meets_activation_requirements(ctx, registrar) {
                    self.state = EntityState::PendingDeactivation {
                        deactivation_start_ts: clock.unix_timestamp,
                        timelock: registrar.deactivation_timelock(),
                    }
                }
            }
        }
    }

    /// Returns true if this Entity is capable of being "activated", i.e., can
    /// enter the staking pool.
    pub fn meets_activation_requirements(&self, ctx: &PoolPrices, registrar: &Registrar) -> bool {
        self.activation_amount(ctx) >= registrar.reward_activation_threshold
            && (self.balances.spt_mega_amount >= 1 || self.balances.current_mega_deposit >= 1)
    }

    pub fn slash(&mut self, spt_amount: u64, mega: bool) {
        if mega {
            self.balances.spt_mega_amount -= spt_amount;
        } else {
            self.balances.spt_amount -= spt_amount;
        }
    }

    pub fn amount_equivalent(&self, prices: &PoolPrices) -> u64 {
        prices.srm_equivalent(self.balances.spt_amount, false)
            + prices.srm_equivalent(self.balances.spt_mega_amount, true)
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

/// PoolPrices represents the current state of the two node staking pools.
///
/// Each Basket represents an exchange ratio of *1* staking pool token
/// for the basket of underlying assets.
#[derive(BorshSerialize, BorshDeserialize, BorshSchema, Clone, Debug)]
pub struct PoolPrices {
    /// `basket` represents the underlying asset Basket for a *single* SRM
    /// staking pool token. It has as single asset: SRM.
    pub basket: Basket,
    /// `mega_basket` represents the underlying asset Basket for a *single* MSRM
    /// staking pool token. It has two assets: MSRM and SRM.
    pub mega_basket: Basket,
}

impl Default for PoolPrices {
    fn default() -> Self {
        PoolPrices {
            basket: Basket {
                quantities: vec![0],
            },
            mega_basket: Basket {
                quantities: vec![0, 0],
            },
        }
    }
}

impl PoolPrices {
    pub fn new(basket: Basket, mega_basket: Basket) -> Self {
        assert!(basket.quantities.len() == 1);
        assert!(mega_basket.quantities.len() == 2);
        Self {
            basket,
            mega_basket,
        }
    }

    /// Returns the amount of SRM the given `spt_amount` staking pool tokens
    /// are worth.
    pub fn srm_equivalent(&self, spt_count: u64, is_mega: bool) -> u64 {
        if is_mega {
            spt_count * self.mega_basket.quantities[0] as u64
                + spt_count * self.mega_basket.quantities[1] as u64 * 1_000_000
        } else {
            spt_count * self.basket.quantities[0] as u64
        }
    }

    pub fn basket_quantities(&self, spt_count: u64, mega: bool) -> Result<Vec<u64>, RegistryError> {
        let basket = {
            if mega {
                &self.mega_basket
            } else {
                &self.basket
            }
        };
        let q: Option<Vec<u64>> = basket
            .quantities
            .iter()
            .map(|q| (*q as u64).checked_mul(spt_count)?.try_into().ok())
            .collect();
        q.ok_or(RegistryErrorCode::CheckedFailure.into())
    }
}
