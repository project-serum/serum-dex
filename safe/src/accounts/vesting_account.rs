use crate::error::{SafeError, SafeErrorCode};
use solana_client_gen::solana_sdk::pubkey::Pubkey;

#[derive(Default, Debug, serde::Serialize, serde::Deserialize)]
pub struct VestingAccount {
    /// The safe instance this account is associated with.
    pub safe: Pubkey,
    /// The *effective* owner of this VestingAccount. The beneficiary
    /// can mint lSRM and withdraw vested SRM to SPL accounts where
    /// the owner of those SPL accounts matches this beneficiary.
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

impl VestingAccount {
    /// Returns the size of the account's data array, assuming it has `slot_account`
    /// vesting periods.
    pub fn data_size(slot_count: usize) -> Result<u64, SafeError> {
        let mut d: VestingAccount = Default::default();
        d.slots = vec![0; slot_count];
        d.amounts = vec![0; slot_count];
        serum_common::pack::bytes_size(&d)
            .map_err(|_| SafeError::ErrorCode(SafeErrorCode::SizeNotAvailable))
    }

    /// Returns the total deposit in this vesting account.
    pub fn total(&self) -> u64 {
        self.amounts.iter().sum()
    }

    /// Returns the total vested amount up to the given slot. This is not
    /// necessarily available for withdrawal.
    pub fn vested_amount(&self, slot: u64) -> u64 {
        self.slots
            .iter()
            .filter_map(|s| if *s <= slot { Some(s) } else { None })
            .enumerate()
            .map(|(idx, _slot)| self.amounts[idx])
            .sum()
    }

    /// Returns the amount available for withdrawal as of the given slot.
    pub fn available_for_withdrawal(&self, slot: u64) -> u64 {
        self.vested_amount(slot) - self.locked_outstanding
    }

    /// Deducts the given amount from the vesting account from the earliest
    /// vesting slots.
    pub fn deduct(&mut self, mut amount: u64) {
        for k in 0..self.amounts.len() {
            if amount < self.amounts[k] {
                self.amounts[k] -= amount;
                return;
            } else if amount == self.amounts[k] {
                self.amounts[k] = 0;
                return;
            } else {
                let old = self.amounts[k];
                self.amounts[k] = 0;
                amount -= old;
            }
        }
    }
}

serum_common::packable!(VestingAccount);

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
        let vesting_account = VestingAccount {
            safe,
            beneficiary,
            initialized,
            locked_outstanding,
            amounts: amounts.clone(),
            slots: slots.clone(),
        };

        // When I pack it into a slice.
        let mut dst = vec![];
        dst.resize(
            VestingAccount::data_size(slots.len()).unwrap() as usize,
            0u8,
        );
        VestingAccount::pack(vesting_account, &mut dst).unwrap();

        // Then I can unpack it from a slice.
        let va = VestingAccount::unpack(&dst).unwrap();
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
        let vesting_account = VestingAccount {
            safe,
            beneficiary,
            initialized,
            locked_outstanding,
            amounts: amounts.clone(),
            slots: slots.clone(),
        };
        assert_eq!(0, vesting_account.available_for_withdrawal(4));
        assert_eq!(1, vesting_account.available_for_withdrawal(5));
        assert_eq!(3, vesting_account.available_for_withdrawal(6));
        assert_eq!(10, vesting_account.available_for_withdrawal(8));
        assert_eq!(10, vesting_account.available_for_withdrawal(100));
    }
}
