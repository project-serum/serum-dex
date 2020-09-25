use serum_common::pack::*;
use solana_client_gen::solana_sdk::pubkey::Pubkey;

#[cfg(feature = "client")]
lazy_static::lazy_static! {
    pub static ref SIZE: u64 = Vesting::default()
                .size()
                .expect("Vesting has a fixed size");
}

/// The Vesting account represents a single deposit of a token
/// available for withdrawal over a period of time determined by
/// a vesting schedule.
#[derive(Default, Debug, serde::Serialize, serde::Deserialize)]
pub struct Vesting {
    /// The Safe instance this account is associated with.
    pub safe: Pubkey,
    /// The *effective* owner of this Vesting account.
    pub beneficiary: Pubkey,
    /// True iff the vesting account has been initialized via deposit.
    pub initialized: bool,
    /// The amount of locked SRM minted and in circulation.
    pub locked_outstanding: u64,
    /// The outstanding SRM deposit backing this vesting account.
    pub balance: u64,
    /// The starting balance of this vesting account, i.e., how much was
    /// originally deposited.
    pub start_balance: u64,
    /// The slot at which this vesting account was created.
    pub start_slot: u64,
    /// The slot at which all the tokens associated with this account
    /// should be vested.
    pub end_slot: u64,
    /// The number of times vesting will occur. For example, if vesting
    /// is once a year over seven years, this will be 7.
    pub period_count: u64,
}

impl Vesting {
    /// Deducts the given amount from the vesting account upon withdrawal.
    pub fn deduct(&mut self, amount: u64) {
        self.balance -= amount;
    }

    /// Returns the amount available for minting locked token NFTs.
    pub fn available_for_mint(&self) -> u64 {
        self.balance - self.locked_outstanding
    }

    /// Returns the amount available for withdrawal as of the given slot.
    pub fn available_for_withdrawal(&self, current_slot: u64) -> u64 {
        std::cmp::min(self.balance_vested(current_slot), self.available_for_mint())
    }

    // The outstanding SRM deposit associated with this account that has not
    // been withdraw. Does not consider outstanding lSRM in circulation.
    fn balance_vested(&self, current_slot: u64) -> u64 {
        self.total_vested(current_slot) - self.withdrawn_amount()
    }

    // Returns the total vested amount up to the given slot.
    fn total_vested(&self, current_slot: u64) -> u64 {
        assert!(current_slot >= self.start_slot);

        if current_slot >= self.end_slot {
            return self.start_balance;
        }
        self.linear_unlock(current_slot)
    }

    // Returns the amount withdrawn from this vesting account.
    fn withdrawn_amount(&self) -> u64 {
        self.start_balance - self.balance
    }

    fn linear_unlock(&self, current_slot: u64) -> u64 {
        let (end_slot, start_slot) = {
            // If we can't perfectly partition the vesting window,
            // push the start window back so that we can.
            //
            // This has the effect of making the first vesting period act as
            // a minor "cliff" that vests slightly more than the rest of the
            // periods.
            let overflow = (self.end_slot - self.start_slot) % self.period_count;
            if overflow != 0 {
                (self.end_slot, self.start_slot - overflow)
            } else {
                (self.end_slot, self.start_slot)
            }
        };

        let vested_period_count = {
            let period = (end_slot - start_slot) / self.period_count;
            let current_period_count = (current_slot - start_slot) / period;
            std::cmp::min(current_period_count, self.period_count)
        };
        let reward_per_period = self.start_balance / self.period_count;

        return vested_period_count * reward_per_period;
    }
}

serum_common::packable!(Vesting);

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::OsRng;
    use solana_client_gen::solana_sdk::signature::{Keypair, Signer};

    #[test]
    fn vesting_account_pack_unpack() {
        // Given a vesting account.
        let safe = Keypair::generate(&mut OsRng).pubkey();
        let beneficiary = Keypair::generate(&mut OsRng).pubkey();
        let initialized = true;
        let locked_outstanding = 99;
        let start_balance = 10;
        let balance = start_balance;
        let start_slot = 11;
        let end_slot = 12;
        let period_count = 13;
        let vesting_acc = Vesting {
            safe,
            beneficiary,
            initialized,
            locked_outstanding,
            balance,
            start_balance,
            start_slot,
            end_slot,
            period_count,
        };

        // When I pack it into a slice.
        let mut dst = vec![];
        dst.resize(Vesting::default().size().unwrap() as usize, 0u8);
        Vesting::pack(vesting_acc, &mut dst).unwrap();

        // Then I can unpack it from a slice.
        let va = Vesting::unpack(&dst).unwrap();
        assert_eq!(va.safe, safe);
        assert_eq!(va.beneficiary, beneficiary);
        assert_eq!(va.locked_outstanding, locked_outstanding);
        assert_eq!(va.initialized, initialized);
        assert_eq!(va.start_balance, start_balance);
        assert_eq!(va.balance, balance);
        assert_eq!(va.start_slot, start_slot);
        assert_eq!(va.end_slot, end_slot);
        assert_eq!(va.period_count, period_count);
    }

    #[test]
    fn available_for_withdrawal() {
        let safe = Keypair::generate(&mut OsRng).pubkey();
        let beneficiary = Keypair::generate(&mut OsRng).pubkey();
        let balance = 10;
        let start_balance = 10;
        let start_slot = 10;
        let end_slot = 20;
        let period_count = 5;
        let initialized = true;
        let locked_outstanding = 0;
        let vesting_acc = Vesting {
            safe,
            beneficiary,
            initialized,
            locked_outstanding,
            balance,
            start_balance,
            start_slot,
            end_slot,
            period_count,
        };
        assert_eq!(0, vesting_acc.available_for_withdrawal(10));
        assert_eq!(0, vesting_acc.available_for_withdrawal(11));
        assert_eq!(2, vesting_acc.available_for_withdrawal(12));
        assert_eq!(2, vesting_acc.available_for_withdrawal(13));
        assert_eq!(4, vesting_acc.available_for_withdrawal(14));
        assert_eq!(8, vesting_acc.available_for_withdrawal(19));
        assert_eq!(10, vesting_acc.available_for_withdrawal(20));
        assert_eq!(10, vesting_acc.available_for_withdrawal(100));
    }

    #[test]
    fn unpack_zeroes() {
        let og_size = Vesting::default().size().unwrap();
        let zero_data = vec![0; og_size as usize];
        let r = Vesting::unpack(&zero_data).unwrap();
        assert_eq!(r.initialized, false);
        assert_eq!(r.safe, Pubkey::new(&[0; 32]));
        assert_eq!(r.beneficiary, Pubkey::new(&[0; 32]));
        assert_eq!(r.locked_outstanding, 0);
        assert_eq!(r.balance, 0);
        assert_eq!(r.start_slot, 0);
        assert_eq!(r.end_slot, 0);
        assert_eq!(r.period_count, 0);
    }
}
