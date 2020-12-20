use borsh::{BorshDeserialize, BorshSchema, BorshSerialize};
use serum_common::pack::*;
use solana_client_gen::prelude::*;

#[cfg(feature = "client")]
lazy_static::lazy_static! {
    pub static ref SIZE: u64 = Vesting::default()
                .size()
                .expect("Vesting has a fixed size");
}

#[derive(Debug, Default, BorshSerialize, BorshDeserialize, BorshSchema)]
pub struct Vesting {
    /// True iff the vesting account has been initialized via deposit.
    pub initialized: bool,
    /// The Safe instance this account is associated with.
    pub safe: Pubkey,
    /// The owner of this Vesting account. If not set, then the account
    /// is allocated but needs to be assigned.
    pub beneficiary: Pubkey,
    /// The mint of the SPL token locked up.
    pub mint: Pubkey,
    /// Address of the account's token vault.
    pub vault: Pubkey,
    /// The owner of the token account funding this account.
    pub grantor: Pubkey,
    /// The outstanding SRM deposit backing this vesting account. All
    /// withdrawals will deduct this balance.
    pub outstanding: u64,
    /// The starting balance of this vesting account, i.e., how much was
    /// originally deposited.
    pub start_balance: u64,
    /// The unix timestamp at which this vesting account was created.
    pub start_ts: i64,
    /// The ts at which all the tokens associated with this account
    /// should be vested.
    pub end_ts: i64,
    /// The number of times vesting will occur. For example, if vesting
    /// is once a year over seven years, this will be 7.
    pub period_count: u64,
    /// The amount of tokens in custody of whitelisted programs.
    pub whitelist_owned: u64,
    /// Signer nonce.
    pub nonce: u8,
}

impl Vesting {
    pub fn available_for_withdrawal(&self, current_ts: i64) -> u64 {
        std::cmp::min(self.outstanding_vested(current_ts), self.balance())
    }

    // The amount of funds currently in the vault.
    pub fn balance(&self) -> u64 {
        self.outstanding.checked_sub(self.whitelist_owned).unwrap()
    }

    // The amount of outstanding locked tokens vested. Note that these
    // tokens might have been transferred to whitelisted programs.
    fn outstanding_vested(&self, current_ts: i64) -> u64 {
        self.total_vested(current_ts)
            .checked_sub(self.withdrawn_amount())
            .unwrap()
    }

    // Returns the amount withdrawn from this vesting account.
    fn withdrawn_amount(&self) -> u64 {
        self.start_balance.checked_sub(self.outstanding).unwrap()
    }

    // Returns the total vested amount up to the given ts, assuming zero
    // withdrawals and zero funds sent to other programs.
    fn total_vested(&self, current_ts: i64) -> u64 {
        assert!(current_ts >= self.start_ts);

        if current_ts >= self.end_ts {
            return self.start_balance;
        }
        self.linear_unlock(current_ts).unwrap()
    }

    fn linear_unlock(&self, current_ts: i64) -> Option<u64> {
        // Signed division not supported.
        let current_ts = current_ts as u64;
        let start_ts = self.start_ts as u64;
        let end_ts = self.end_ts as u64;

        // If we can't perfectly partition the vesting window,
        // push the start of the window back so that we can.
        //
        // This has the effect of making the first vesting period shorter
        // than the rest.
        let shifted_start_ts =
            start_ts.checked_sub(end_ts.checked_sub(start_ts)? % self.period_count)?;

        // Similarly, if we can't perfectly divide up the vesting rewards
        // then make the first period act as a cliff, earning slightly more than
        // subsequent periods.
        let reward_overflow = self.start_balance % self.period_count;

        // Reward per period ignoring the overflow.
        let reward_per_period =
            (self.start_balance.checked_sub(reward_overflow)?).checked_div(self.period_count)?;

        // Number of vesting periods that have passed.
        let current_period = {
            let period_secs =
                (end_ts.checked_sub(shifted_start_ts)?).checked_div(self.period_count)?;
            let current_period_count =
                (current_ts.checked_sub(shifted_start_ts)?).checked_div(period_secs)?;
            std::cmp::min(current_period_count, self.period_count)
        };

        if current_period == 0 {
            return Some(0);
        }

        current_period
            .checked_mul(reward_per_period)?
            .checked_add(reward_overflow)
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
        let start_balance = 10;
        let outstanding = start_balance;
        let start_ts = 11;
        let end_ts = 12;
        let period_count = 13;
        let whitelist_owned = 14;
        let grantor = Pubkey::new_unique();
        let mint = Pubkey::new_unique();
        let vault = Pubkey::new_unique();
        let nonce = 0;
        let vesting_acc = Vesting {
            safe,
            beneficiary,
            initialized,
            outstanding,
            start_balance,
            start_ts,
            end_ts,
            period_count,
            whitelist_owned,
            grantor,
            mint,
            nonce,
            vault,
        };

        // When I pack it into a slice.
        let mut dst = vec![];
        dst.resize(Vesting::default().size().unwrap() as usize, 0u8);
        Vesting::pack_unchecked(vesting_acc, &mut dst).unwrap();

        // Then I can unpack it from a slice.
        let mut data: &[u8] = &dst;
        let va = Vesting::unpack_unchecked(&mut data).unwrap();
        assert_eq!(va.safe, safe);
        assert_eq!(va.beneficiary, beneficiary);
        assert_eq!(va.initialized, initialized);
        assert_eq!(va.start_balance, start_balance);
        assert_eq!(va.outstanding, outstanding);
        assert_eq!(va.start_ts, start_ts);
        assert_eq!(va.end_ts, end_ts);
        assert_eq!(va.period_count, period_count);
        assert_eq!(va.whitelist_owned, whitelist_owned);
        assert_eq!(va.grantor, grantor);
    }

    #[test]
    fn available_for_withdrawal() {
        let safe = Keypair::generate(&mut OsRng).pubkey();
        let beneficiary = Keypair::generate(&mut OsRng).pubkey();
        let outstanding = 10;
        let start_balance = 10;
        let start_ts = 10;
        let end_ts = 20;
        let period_count = 5;
        let initialized = true;
        let whitelist_owned = 0;
        let grantor = Pubkey::new_unique();
        let mint = Pubkey::new_unique();
        let vault = Pubkey::new_unique();
        let nonce = 0;
        let vesting_acc = Vesting {
            safe,
            beneficiary,
            initialized,
            whitelist_owned,
            outstanding,
            start_balance,
            start_ts,
            end_ts,
            period_count,
            grantor,
            mint,
            nonce,
            vault,
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
    fn available_for_withdrawal_cliff() {
        let safe = Keypair::generate(&mut OsRng).pubkey();
        let beneficiary = Keypair::generate(&mut OsRng).pubkey();
        let outstanding = 11;
        let start_balance = 11;
        let start_ts = 10;
        let end_ts = 20;
        let period_count = 10;
        let initialized = true;
        let whitelist_owned = 0;
        let grantor = Pubkey::new_unique();
        let mint = Pubkey::new_unique();
        let vault = Pubkey::new_unique();
        let nonce = 0;
        let vesting_acc = Vesting {
            safe,
            beneficiary,
            initialized,
            whitelist_owned,
            outstanding,
            start_balance,
            start_ts,
            end_ts,
            period_count,
            grantor,
            mint,
            nonce,
            vault,
        };
        assert_eq!(0, vesting_acc.available_for_withdrawal(10));
        assert_eq!(2, vesting_acc.available_for_withdrawal(11));
        assert_eq!(3, vesting_acc.available_for_withdrawal(12));
        assert_eq!(4, vesting_acc.available_for_withdrawal(13));
        assert_eq!(5, vesting_acc.available_for_withdrawal(14));
        assert_eq!(6, vesting_acc.available_for_withdrawal(15));
        assert_eq!(7, vesting_acc.available_for_withdrawal(16));
        assert_eq!(8, vesting_acc.available_for_withdrawal(17));
        assert_eq!(9, vesting_acc.available_for_withdrawal(18));
        assert_eq!(10, vesting_acc.available_for_withdrawal(19));
        assert_eq!(11, vesting_acc.available_for_withdrawal(20));
        assert_eq!(11, vesting_acc.available_for_withdrawal(21));
        assert_eq!(11, vesting_acc.available_for_withdrawal(2100));
    }

    #[test]
    fn unpack_zeroes() {
        let og_size = Vesting::default().size().unwrap();
        let d = vec![0; og_size as usize];
        let mut zero_data: &[u8] = &d;
        let r = Vesting::unpack_unchecked(&mut zero_data).unwrap();
        assert_eq!(r.initialized, false);
        assert_eq!(r.safe, Pubkey::new(&[0; 32]));
        assert_eq!(r.beneficiary, Pubkey::new(&[0; 32]));
        assert_eq!(r.outstanding, 0);
        assert_eq!(r.start_ts, 0);
        assert_eq!(r.end_ts, 0);
        assert_eq!(r.period_count, 0);
        assert_eq!(r.whitelist_owned, 0);
    }
}
