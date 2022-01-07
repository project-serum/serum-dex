use num_enum::{IntoPrimitive, TryFromPrimitive};
use solana_program::pubkey::Pubkey;
use std::convert::TryInto;

#[cfg(test)]
use proptest_derive::Arbitrary;

mod stable_markets {
    pub mod usdt_usdc {
        solana_program::declare_id!("77quYg4MGneUdjgXCunt9GgM1usmrxKY31twEy3WHwcS");
    }
    pub mod msol_sol {
        solana_program::declare_id!("5cLrMai1DsLRYc1Nio9qMTicsWtvzjzZfJPXyAoF4t1Z");
    }
}

#[derive(Copy, Clone, IntoPrimitive, TryFromPrimitive, Debug)]
#[cfg_attr(test, derive(Arbitrary))]
#[repr(u8)]
pub enum FeeTier {
    Base,
    SRM2,
    SRM3,
    SRM4,
    SRM5,
    SRM6,
    MSRM,
    Stable,
}

#[repr(transparent)]
#[derive(Copy, Clone)]
struct U64F64(u128);

impl U64F64 {
    const ONE: Self = U64F64(1 << 64);

    #[inline(always)]
    const fn add(self, other: U64F64) -> U64F64 {
        U64F64(self.0 + other.0)
    }

    #[inline(always)]
    const fn div(self, other: U64F64) -> u128 {
        self.0 / other.0
    }

    #[inline(always)]
    const fn mul_u64(self, other: u64) -> U64F64 {
        U64F64(self.0 * other as u128)
    }

    #[inline(always)]
    const fn floor(self) -> u64 {
        (self.0 >> 64) as u64
    }

    #[inline(always)]
    const fn frac_part(self) -> u64 {
        self.0 as u64
    }

    #[inline(always)]
    const fn from_int(n: u64) -> Self {
        U64F64((n as u128) << 64)
    }
}

#[inline(always)]
const fn fee_tenth_of_bps(tenth_of_bps: u64) -> U64F64 {
    U64F64(((tenth_of_bps as u128) << 64) / 100_000)
}

#[inline(always)]
const fn rebate_tenth_of_bps(tenth_of_bps: u64) -> U64F64 {
    U64F64(fee_tenth_of_bps(tenth_of_bps).0 + 1)
}

impl FeeTier {
    #[inline]
    pub fn from_srm_and_msrm_balances(market: &Pubkey, srm_held: u64, msrm_held: u64) -> FeeTier {
        let one_srm = 1_000_000;

        if market == &stable_markets::usdt_usdc::ID || market == &stable_markets::msol_sol::ID {
            return FeeTier::Stable;
        }

        match () {
            () if msrm_held >= 1 => FeeTier::MSRM,
            () if srm_held >= one_srm * 1_000_000 => FeeTier::SRM6,
            () if srm_held >= one_srm * 100_000 => FeeTier::SRM5,
            () if srm_held >= one_srm * 10_000 => FeeTier::SRM4,
            () if srm_held >= one_srm * 1_000 => FeeTier::SRM3,
            () if srm_held >= one_srm * 100 => FeeTier::SRM2,
            () => FeeTier::Base,
        }
    }

    #[inline]
    pub fn maker_rebate(self, pc_qty: u64) -> u64 {
        rebate_tenth_of_bps(0).mul_u64(pc_qty).floor()
    }

    fn taker_rate(self) -> U64F64 {
        use FeeTier::*;
        match self {
            Base => fee_tenth_of_bps(40),
            SRM2 => fee_tenth_of_bps(39),
            SRM3 => fee_tenth_of_bps(38),
            SRM4 => fee_tenth_of_bps(36),
            SRM5 => fee_tenth_of_bps(34),
            SRM6 => fee_tenth_of_bps(32),
            MSRM => fee_tenth_of_bps(30),
            Stable => fee_tenth_of_bps(10),
        }
    }

    #[inline]
    pub fn taker_fee(self, pc_qty: u64) -> u64 {
        let rate = self.taker_rate();
        let exact_fee: U64F64 = rate.mul_u64(pc_qty);
        exact_fee.floor() + ((exact_fee.frac_part() != 0) as u64)
    }

    #[inline]
    pub fn remove_taker_fee(self, pc_qty_incl_fee: u64) -> u64 {
        let rate = self.taker_rate();
        U64F64::from_int(pc_qty_incl_fee)
            .div(U64F64::ONE.add(rate))
            .try_into()
            .unwrap()
    }
}

#[inline]
pub fn referrer_rebate(amount: u64) -> u64 {
    amount / 5
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn positive_net_fees(tt: FeeTier, mt: FeeTier, qty in 1..=std::u64::MAX) {
            let fee = tt.taker_fee(qty);
            let rebate = mt.maker_rebate(qty) + referrer_rebate(fee);
            assert!(fee > rebate);
            let net_bps_u64f64 = (fee - rebate) as u128 * 100_000;
            let three_bps = (qty as u128) * 3;
            let dust_qty_u64f64 = 1 << 32;
            assert!(net_bps_u64f64 + dust_qty_u64f64 > three_bps, "{:x}, {:x}, {:x}", qty, net_bps_u64f64, three_bps);
        }

        #[test]
        fn fee_tenth_of_bps_approx(tenth_of_bps in 1..1000u64) {
            let rate = fee_tenth_of_bps(tenth_of_bps);
            let rate_bps: U64F64 = rate.mul_u64(100_000);
            let rate_bps_int: u64 = rate_bps.floor();
            let rate_bps_frac: u64 = rate_bps.frac_part();
            let inexact = rate_bps_frac != 0;
            assert!(rate_bps_int == tenth_of_bps - (inexact as u64));
        }

        #[test]
        fn market_order_cannot_cheat(tier: FeeTier, qty: u64) {
            let qty_without_fees = tier.remove_taker_fee(qty);
            let required_fee = tier.taker_fee(qty_without_fees) as i128;
            let actual_fee = qty as i128 - qty_without_fees as i128;
            assert!([required_fee + 1, required_fee].contains(&actual_fee),
                    "actual_fee = {}, required_fee = {}",
                    actual_fee, required_fee);
        }

        #[test]
        fn test_add_remove_fees(tier: FeeTier, qty in 1..=(std::u64::MAX >> 1)) {
            let qty_with_fees = qty + tier.taker_fee(qty);
            let qty2 = tier.remove_taker_fee(qty_with_fees);
            assert!([-1, 0, 1].contains(&(qty as i128 - qty2 as i128)))
        }
    }
}
