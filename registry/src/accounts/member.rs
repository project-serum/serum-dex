use crate::accounts::entity::PoolPrices;
use crate::error::{RegistryError, RegistryErrorCode};
use borsh::{BorshDeserialize, BorshSchema, BorshSerialize};
use serum_common::pack::*;
use solana_client_gen::solana_sdk::pubkey::Pubkey;

#[cfg(feature = "client")]
lazy_static::lazy_static! {
    pub static ref SIZE: u64 = Member::default()
                .size()
                .expect("Member has a fixed size");
}

#[derive(Default, Debug, BorshSerialize, BorshDeserialize)]
pub struct Member {
    /// Set by the program on creation.
    pub initialized: bool,
    /// Registrar the member belongs to.
    pub registrar: Pubkey,
    /// The effective owner of the Member account.
    pub beneficiary: Pubkey,
    /// Entity providing membership.
    pub entity: Pubkey,
    /// The entity's activation counter to which the Member stake belongs.
    pub generation: u64,
    /// SRM, MSRM, and staking pool token balances.
    pub balances: MemberBalances,
    /// The *last* stake context used when creating a staking pool token.
    /// This is used as a fallback mechanism, to mark the price of a staking
    /// pool token when a withdrawal on an inactive entity happens *and*
    /// no `Generation` is provided to the stake withdrawal instruction.
    pub last_active_prices: PoolPrices,
    /// Arbitrary metadata account owned by any program.
    pub metadata: Pubkey,
    /// Staking pool token account.
    pub spt: Pubkey,
    /// Mega staking pool token account.
    pub spt_mega: Pubkey,
    /// Next position in the rewards event queue to process.
    pub rewards_cursor: u32,
    /// The clock timestamp of the last time this account staked/unstaked.
    pub last_stake_ts: i64,
}

impl Member {
    pub fn can_afford(
        &self,
        prices: &PoolPrices,
        spt_amount: u64,
        mega: bool,
    ) -> Result<bool, RegistryError> {
        let purchase_price = prices.basket_quantities(spt_amount, mega)?;

        if self.balances.current_deposit < purchase_price[0] {
            return Err(RegistryErrorCode::InsufficientStakeIntentBalance)?;
        }
        if mega {
            if self.balances.current_mega_deposit < purchase_price[1] {
                return Err(RegistryErrorCode::InsufficientStakeIntentBalance)?;
            }
        }
        Ok(true)
    }

    pub fn can_withdraw(
        &self,
        prices: &PoolPrices,
        amount: u64,
        mega: bool,
        owner: Pubkey,
    ) -> Result<bool, RegistryError> {
        let delegate = self.balances.delegate.owner == owner;

        // Current valuation of our staking tokens for both pools.
        let basket = prices.basket_quantities(self.balances.spt_amount, false)?;
        let mega_basket = prices.basket_quantities(self.balances.spt_mega_amount, true)?;

        // In both cases, we need to be able to 1) cover the withdrawal
        // with our *current* stake intent vault balances and also
        // cover any future withdrawals needed to cover the cost basis
        // of the delegate account. That is, all locked SRM/MSRM coming into the
        // program must eventually go back.
        if mega {
            if amount > self.balances.current_mega_deposit {
                return Err(RegistryErrorCode::InsufficientStakeIntentBalance)?;
            }
            if !delegate {
                let remaining_msrm = mega_basket[1] + self.balances.current_mega_deposit - amount;
                if remaining_msrm < self.balances.delegate.mega_deposit {
                    return Err(RegistryErrorCode::InsufficientBalance)?;
                }
            }
        } else {
            if amount > self.balances.current_deposit {
                return Err(RegistryErrorCode::InsufficientStakeIntentBalance)?;
            }
            if !delegate {
                let remaining_srm =
                    basket[0] + mega_basket[0] + self.balances.current_deposit - amount;
                if remaining_srm < self.balances.delegate.deposit {
                    return Err(RegistryErrorCode::InsufficientBalance)?;
                }
            }
        }

        Ok(true)
    }

    pub fn stake_is_empty(&self) -> bool {
        self.balances.spt_amount == 0 && self.balances.spt_mega_amount == 0
    }

    pub fn set_delegate(&mut self, delegate: Pubkey) {
        assert!(self.balances.delegate.deposit == 0);
        assert!(self.balances.delegate.mega_deposit == 0);
        self.balances.delegate = OriginalDeposit::new(delegate);
    }

    pub fn did_deposit(&mut self, amount: u64, mega: bool, owner: Pubkey) {
        if mega {
            self.balances.current_mega_deposit += amount;
        } else {
            self.balances.current_deposit += amount;
        }

        let delegate = owner == self.balances.delegate.owner;
        if delegate {
            if mega {
                self.balances.delegate.mega_deposit += amount;
            } else {
                self.balances.delegate.deposit += amount;
            }
        } else {
            if mega {
                self.balances.main.mega_deposit += amount;
            } else {
                self.balances.main.deposit += amount;
            }
        }
    }

    pub fn did_withdraw(&mut self, amount: u64, mega: bool, owner: Pubkey) {
        if mega {
            self.balances.current_mega_deposit -= amount;
        } else {
            self.balances.current_deposit -= amount;
        }

        let delegate = owner == self.balances.delegate.owner;
        if delegate {
            if mega {
                self.balances.delegate.mega_deposit -= amount;
            } else {
                self.balances.delegate.deposit -= amount;
            }
        } else {
            if mega {
                self.balances.main.mega_deposit -= amount;
            } else {
                self.balances.main.deposit -= amount;
            }
        }
    }

    pub fn spt_did_create(
        &mut self,
        prices: &PoolPrices,
        amount: u64,
        mega: bool,
    ) -> Result<(), RegistryError> {
        if mega {
            self.balances.spt_mega_amount += amount;

            let basket = prices.basket_quantities(amount, mega)?;
            self.balances.current_deposit -= basket[0];
            self.balances.current_mega_deposit -= basket[1];

            // Only modify the prices of the basket the member is creating.
            self.last_active_prices.mega_basket = prices.mega_basket.clone();
        } else {
            self.balances.spt_amount += amount;

            let basket = prices.basket_quantities(amount, mega)?;
            self.balances.current_deposit -= basket[0];

            // Only modify the prices of the basket the member is creating.
            self.last_active_prices.basket = prices.basket.clone();
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

    pub fn slash(&mut self, spt_amount: u64, mega: bool) {
        if mega {
            self.balances.spt_mega_amount -= spt_amount;
        } else {
            self.balances.spt_amount -= spt_amount;
        }
    }
}

serum_common::packable!(Member);

#[derive(Default, Debug, BorshSerialize, BorshDeserialize, BorshSchema)]
pub struct MemberBalances {
    // The amount of SPT tokens in the SRM pool.
    pub spt_amount: u64,
    // The amount of SPT tokens in the MSRM pool.
    pub spt_mega_amount: u64,
    // SRM in the current_deposit vault.
    pub current_deposit: u64,
    // MSRM in the current_deposit vault.
    pub current_mega_deposit: u64,
    // Original deposit.
    pub main: OriginalDeposit,
    // Original deposit from delegate.
    pub delegate: OriginalDeposit,
}

impl MemberBalances {
    pub fn new(beneficiary: Pubkey, delegate: Pubkey) -> Self {
        Self {
            spt_amount: 0,
            spt_mega_amount: 0,
            current_deposit: 0,
            current_mega_deposit: 0,
            main: OriginalDeposit::new(beneficiary),
            delegate: OriginalDeposit::new(delegate),
        }
    }

    pub fn stake_is_empty(&self) -> bool {
        self.spt_amount + self.spt_mega_amount == 0
    }
}

// OriginalDeposit tracks the amount of tokens originally deposited into a Member
// account. These funds might be in either the deposit vault or the pool.
//
// It is used to track the amount of funds that must be returned to delegate
// programs, e.g., the lockup program.
#[derive(Default, Debug, BorshSerialize, BorshDeserialize, BorshSchema)]
pub struct OriginalDeposit {
    pub owner: Pubkey,
    pub deposit: u64,
    pub mega_deposit: u64,
}

impl OriginalDeposit {
    pub fn new(owner: Pubkey) -> Self {
        Self {
            owner,
            deposit: 0,
            mega_deposit: 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.deposit + self.mega_deposit == 0
    }
}
