use borsh::{BorshDeserialize, BorshSchema, BorshSerialize};
use serum_common::pack::*;
use solana_client_gen::prelude::*;

#[cfg(feature = "client")]
lazy_static::lazy_static! {
    pub static ref SIZE: u64 = Vesting::default()
                .size()
                .expect("Vesting has a fixed size");
}

/// The Vesting account represents a single deposit of a token
/// available for withdrawal over a period of time determined by
/// a vesting schedule.
#[derive(Default, Debug, BorshSerialize, BorshDeserialize, BorshSchema)]
pub struct Vesting {
    /// True iff the vesting account has been initialized via deposit.
    pub initialized: bool,
    /// One time token for claiming the vesting account.
    pub claimed: bool,
    /// The Safe instance this account is associated with.
    pub safe: Pubkey,
    /// The effective owner of this Vesting account.
    pub beneficiary: Pubkey,
    /// The outstanding SRM deposit backing this vesting account. All
    /// withdrawals/redemptions will deduct this balance.
    pub balance: u64,
    /// The starting balance of this vesting account, i.e., how much was
    /// originally deposited.
    pub start_balance: u64,
    /// The ts at which this vesting account was created.
    pub start_ts: i64,
    /// The ts at which all the tokens associated with this account
    /// should be vested.
    pub end_ts: i64,
    /// The number of times vesting will occur. For example, if vesting
    /// is once a year over seven years, this will be 7.
    pub period_count: u64,
    /// The spl token mint associated with this vesting account. The supply
    /// should always equal the `balance` field.
    pub locked_nft_mint: Pubkey,
    /// The token account the locked_nft supply is expected to be in.
    pub locked_nft_token: Pubkey,
    /// The amount of tokens in custody of whitelisted programs.
    pub whitelist_owned: u64,
}

impl Vesting {
    /// Deducts the given amount from the vesting account upon
    /// withdrawal/redemption.
    pub fn deduct(&mut self, amount: u64) {
        self.balance -= amount;
    }

    /// Returns the amount available for withdrawal as of the given ts.
    /// The amount for withdrawal is not necessarily the balance vested
    /// since funds can be sent to whitelisted programs. For this reason,
    /// take minimum of the availble balance vested and the available balance
    /// for sending to whitelisted programs.
    pub fn available_for_withdrawal(&self, current_ts: i64) -> u64 {
        std::cmp::min(
            self.balance_vested(current_ts),
            self.available_for_whitelist(),
        )
    }

    /// Amount available for whitelisted programs to transfer.
    pub fn available_for_whitelist(&self) -> u64 {
        self.balance - self.whitelist_owned
    }

    // The amount vested that's available for withdrawal, if no funds were ever
    // sent to another program.
    fn balance_vested(&self, current_ts: i64) -> u64 {
        self.total_vested(current_ts) - self.withdrawn_amount()
    }

    // Returns the total vested amount up to the given ts, assuming zero
    // withdrawals and zero funds sent to other programs.
    fn total_vested(&self, current_ts: i64) -> u64 {
        assert!(current_ts >= self.start_ts);

        if current_ts >= self.end_ts {
            return self.start_balance;
        }
        self.linear_unlock(current_ts)
    }

    // Returns the amount withdrawn from this vesting account.
    fn withdrawn_amount(&self) -> u64 {
        self.start_balance - self.balance
    }

    fn linear_unlock(&self, current_ts: i64) -> u64 {
        let (end_ts, start_ts) = {
            // If we can't perfectly partition the vesting window,
            // push the start window back so that we can.
            //
            // This has the effect of making the first vesting period act as
            // a minor "cliff" that vests slightly more than the rest of the
            // periods.
            let overflow = (self.end_ts - self.start_ts) as u64 % self.period_count;
            if overflow != 0 {
                (self.end_ts, self.start_ts - overflow as i64)
            } else {
                (self.end_ts, self.start_ts)
            }
        };

        let vested_period_count = {
            let period = (end_ts - start_ts) as u64 / self.period_count;
            let current_period_count = (current_ts - start_ts) as u64 / period;
            std::cmp::min(current_period_count, self.period_count)
        };
        let reward_per_period = self.start_balance / self.period_count;

        vested_period_count * reward_per_period
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
        let balance = start_balance;
        let start_ts = 11;
        let end_ts = 12;
        let period_count = 13;
        let locked_nft_mint = Pubkey::new_rand();
        let whitelist_owned = 14;
        let claimed = true;
        let locked_nft_token = Pubkey::new_rand();
        let vesting_acc = Vesting {
            safe,
            claimed,
            beneficiary,
            initialized,
            balance,
            start_balance,
            start_ts,
            end_ts,
            period_count,
            locked_nft_mint,
            whitelist_owned,
            locked_nft_token,
        };

        // When I pack it into a slice.
        let mut dst = vec![];
        dst.resize(Vesting::default().size().unwrap() as usize, 0u8);
        Vesting::pack(vesting_acc, &mut dst).unwrap();

        // Then I can unpack it from a slice.
        let va = Vesting::unpack(&dst).unwrap();
        assert_eq!(va.safe, safe);
        assert_eq!(va.beneficiary, beneficiary);
        assert_eq!(va.initialized, initialized);
        assert_eq!(va.start_balance, start_balance);
        assert_eq!(va.balance, balance);
        assert_eq!(va.start_ts, start_ts);
        assert_eq!(va.end_ts, end_ts);
        assert_eq!(va.period_count, period_count);
        assert_eq!(va.locked_nft_mint, locked_nft_mint);
        assert_eq!(va.whitelist_owned, whitelist_owned);
        assert_eq!(va.locked_nft_token, locked_nft_token);
    }

    #[test]
    fn available_for_withdrawal() {
        let safe = Keypair::generate(&mut OsRng).pubkey();
        let beneficiary = Keypair::generate(&mut OsRng).pubkey();
        let balance = 10;
        let start_balance = 10;
        let start_ts = 10;
        let end_ts = 20;
        let period_count = 5;
        let initialized = true;
        let locked_nft_mint = Pubkey::new_rand();
        let whitelist_owned = 0;
        let claimed = true;
        let locked_nft_token = Pubkey::new_rand();
        let vesting_acc = Vesting {
            safe,
            claimed,
            beneficiary,
            initialized,
            locked_nft_mint,
            whitelist_owned,
            balance,
            start_balance,
            start_ts,
            end_ts,
            period_count,
            locked_nft_token,
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
        assert_eq!(r.balance, 0);
        assert_eq!(r.start_ts, 0);
        assert_eq!(r.end_ts, 0);
        assert_eq!(r.period_count, 0);
        assert_eq!(r.claimed, false);
        assert_eq!(r.whitelist_owned, 0);
        assert_eq!(r.locked_nft_mint, Pubkey::new_from_array([0; 32]));
    }
}
