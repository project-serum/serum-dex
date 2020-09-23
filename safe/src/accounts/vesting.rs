use crate::error::SafeError;
use solana_client_gen::solana_sdk::pubkey::Pubkey;
use std::cmp::Ordering;

/// The Vesting account represents a single deposit of a token
/// available for withdrawal over a period of time determined by
/// a vesting schedule.
///
/// Note that, unlike other accounts, this account is dynamically
/// sized, which clients must consider when creating these accounts.
/// use the `size_dyn` method to determine how large the account
/// data should be.
#[derive(Default, Debug, serde::Serialize, serde::Deserialize)]
pub struct Vesting {
    /// The Safe instance this account is associated with.
    pub safe: Pubkey,
    /// The *effective* owner of this Vesting account.
    pub beneficiary: Pubkey,
    /// True iff the vesting account has been initialized via deposit.
    pub initialized: bool,
    /// The amount of locked SRM outstanding.
    pub locked_outstanding: u64,
    /// The Solana slots at which each amount vests.
    pub slots: Vec<u64>,
    /// The amount that vests at each slot.
    pub amounts: Vec<u64>,
}

impl Vesting {
    /// Returns the total deposit in this vesting account.
    pub fn total(&self) -> u64 {
        self.amounts.iter().sum()
    }

    /// Returns the amount available for withdrawal as of the given slot.
    pub fn available_for_withdrawal(&self, slot: u64) -> u64 {
        self.vested_amount(slot) - self.locked_outstanding
    }

    /// Returns the total vested amount up to the given slot. This is not
    /// necessarily available for withdrawal.
    pub fn vested_amount(&self, slot: u64) -> u64 {
        self.slots
            .iter()
            .filter(|s| **s <= slot)
            .enumerate()
            .map(|(idx, _slot)| self.amounts[idx])
            .sum()
    }

    /// Deducts the given amount from the vesting account from the earliest
    /// vesting slots.
    pub fn deduct(&mut self, mut amount: u64) {
        for k in 0..self.amounts.len() {
            match amount.cmp(&self.amounts[k]) {
                Ordering::Less => {
                    self.amounts[k] -= amount;
                    return;
                }
                Ordering::Equal => {
                    self.amounts[k] = 0;
                    return;
                }
                Ordering::Greater => {
                    let old = self.amounts[k];
                    self.amounts[k] = 0;
                    amount -= old;
                }
            }
        }
    }

    /// Returns the dynamic size of the account's data array, assuming it has
    /// `slot_account` vesting periods.
    pub fn size_dyn(slot_count: usize) -> Result<u64, SafeError> {
        let mut d: Vesting = Default::default();
        d.slots = vec![0u64; slot_count];
        d.amounts = vec![0u64; slot_count];
        Ok(d.size()?)
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
        let amounts = vec![1, 2, 3, 4];
        let slots = vec![5, 6, 7, 8];
        let initialized = true;
        let locked_outstanding = 99;
        let vesting_acc = Vesting {
            safe,
            beneficiary,
            initialized,
            locked_outstanding,
            amounts: amounts.clone(),
            slots: slots.clone(),
        };

        // When I pack it into a slice.
        let mut dst = vec![];
        dst.resize(Vesting::size_dyn(slots.len()).unwrap() as usize, 0u8);
        Vesting::pack(vesting_acc, &mut dst).unwrap();

        // Then I can unpack it from a slice.
        let va = Vesting::unpack(&dst).unwrap();
        assert_eq!(va.safe, safe);
        assert_eq!(va.beneficiary, beneficiary);
        assert_eq!(va.locked_outstanding, locked_outstanding);

        assert_eq!(va.amounts.len(), amounts.len());
        assert_eq!(va.slots.len(), slots.len());
        let match_amounts = va
            .amounts
            .iter()
            .zip(&amounts)
            .filter(|&(a, b)| a == b)
            .count();
        assert_eq!(va.amounts.len(), match_amounts);
        let match_slots = va.slots.iter().zip(&slots).filter(|&(a, b)| a == b).count();
        assert_eq!(va.amounts.len(), match_slots);
        assert_eq!(va.initialized, initialized);
    }

    #[test]
    fn available_for_withdrawal() {
        let safe = Keypair::generate(&mut OsRng).pubkey();
        let beneficiary = Keypair::generate(&mut OsRng).pubkey();
        let amounts = vec![1, 2, 3, 4];
        let slots = vec![5, 6, 7, 8];
        let initialized = true;
        let locked_outstanding = 0;
        let vesting_acc = Vesting {
            safe,
            beneficiary,
            initialized,
            locked_outstanding,
            amounts: amounts.clone(),
            slots: slots.clone(),
        };
        assert_eq!(0, vesting_acc.available_for_withdrawal(4));
        assert_eq!(1, vesting_acc.available_for_withdrawal(5));
        assert_eq!(3, vesting_acc.available_for_withdrawal(6));
        assert_eq!(10, vesting_acc.available_for_withdrawal(8));
        assert_eq!(10, vesting_acc.available_for_withdrawal(100));
    }

    #[test]
    fn unpack_zeroes_size() {
        let og_size = Vesting::size_dyn(5).unwrap();
        let zero_data = vec![0; og_size as usize];
        let r = Vesting::unpack(&zero_data);
        match r {
            Ok(_) => panic!("expect error"),
            Err(e) => assert_eq!(e, ProgramError::InvalidAccountData),
        }
    }

    #[test]
    fn unpack_unchecked_zeroes_size() {
        let og_size = Vesting::size_dyn(5).unwrap();
        let zero_data = vec![0; og_size as usize];
        let r = Vesting::unpack_unchecked(&zero_data).unwrap();
        assert_eq!(r.initialized, false);
        assert_eq!(r.safe, Pubkey::new(&[0; 32]));
        assert_eq!(r.beneficiary, Pubkey::new(&[0; 32]));
        assert_eq!(r.locked_outstanding, 0);
        // Notice how we lose information here when deserializing from
        // all zeroes.
        assert_eq!(r.slots.len(), 0);
        assert_eq!(r.amounts.len(), 0);
    }
}
