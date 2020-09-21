use crate::pack::DynPack;
use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use byteorder::{ReadBytesExt, WriteBytesExt};
use solana_client_gen::solana_sdk::program_error::ProgramError;
use solana_client_gen::solana_sdk::pubkey::Pubkey;
// TODO: this is in the solana_sdk. Use that version instead.
use spl_token::pack::{IsInitialized, Pack, Sealed};

#[repr(C)]
#[derive(Debug)]
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
    /// Returns the size of the account's data array.
    pub fn data_size(slot_count: usize) -> usize {
        let dynamic_part = 8 * slot_count * 2;
        // Prefix with 8 bytes for the length of the entire data array.
        8 + dynamic_part + VestingAccount::fixed_size()
    }

    /// Returns the size of the struct data, excluding the first 8
    /// bytes for size.
    pub fn struct_size(&self, slot_count: usize) -> usize {
        VestingAccount::data_size(slot_count) - 8
    }

    pub fn fixed_size() -> usize {
        // 2*pubkey.len() + initialized + locked_outstanding
        64 + 1 + 8
    }

    /// Returns the index of the intialized member in the underlying data array.
    pub fn initialized_index() -> usize {
        // 8 + 32 + 32
        return 72;
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

impl IsInitialized for VestingAccount {
    fn is_initialized(&self) -> bool {
        self.initialized
    }
}

impl Sealed for VestingAccount {}

impl DynPack for VestingAccount {
    fn get_packed_len(&self) -> usize {
        VestingAccount::data_size(self.slots.len())
    }

    fn unpack_from_slice(src: &[u8]) -> Result<Self, ProgramError> {
        let dynamic_size = src.len() - VestingAccount::fixed_size();
        if dynamic_size % 2 != 0 {
            return Err(ProgramError::Custom(9));
        }
        let slot_size = dynamic_size / 2;
        let amount_size = slot_size;

        if dynamic_size % 8 != 0 {
            return Err(ProgramError::Custom(13));
        }

        let src_fixed = array_ref![src, 0, 73];
        let (safe, beneficiary, initialized, locked_outstanding) =
            array_refs![src_fixed, 32, 32, 1, 8];

        let slots = {
            let slots_start = src[VestingAccount::fixed_size()..].to_vec();
            let slots_size_u64 = slot_size / 8;
            let mut slots_rdr = std::io::Cursor::new(slots_start);
            let mut slots = Vec::with_capacity(slots_size_u64);
            for _ in 0..slots_size_u64 {
                let s = slots_rdr.read_u64::<byteorder::LittleEndian>().unwrap();
                slots.push(s);
            }
            slots
        };

        let amounts = {
            let amounts_start = src[VestingAccount::fixed_size() + slot_size..].to_vec();
            let amounts_size_u64 = slot_size / 8;
            let mut amounts_rdr = std::io::Cursor::new(amounts_start);
            let mut amounts = Vec::with_capacity(amounts_size_u64);
            for _ in 0..amounts_size_u64 {
                let a = amounts_rdr.read_u64::<byteorder::LittleEndian>().unwrap();
                amounts.push(a);
            }
            amounts
        };

        Ok(VestingAccount {
            safe: Pubkey::new(safe),
            beneficiary: Pubkey::new(beneficiary),
            initialized: match initialized {
                [0] => false,
                [1] => true,
                _ => return Err(ProgramError::Custom(14)),
            },
            locked_outstanding: u64::from_le_bytes(*locked_outstanding),
            slots,
            amounts,
        })
    }
    fn pack_into_slice(&self, dst: &mut [u8]) {
        let (safe_dst, beneficiary_dst, initialized_dst, locked_outstanding_dst, dynamic_dst) =
            mut_array_refs![dst, 32, 32, 1, 8; .. ;];

        let VestingAccount {
            safe,
            beneficiary,
            initialized,
            slots,
            amounts,
            locked_outstanding,
        } = self;

        safe_dst.copy_from_slice(safe.as_ref());
        beneficiary_dst.copy_from_slice(beneficiary.as_ref());
        initialized_dst[0] = *initialized as u8;
        locked_outstanding_dst.copy_from_slice(&locked_outstanding.to_le_bytes());

        let slots_size = (dynamic_dst.len() / 2) / 8;
        let mut dyn_writer = std::io::Cursor::new(dynamic_dst);

        for k in 0..slots_size {
            dyn_writer
                .write_u64::<byteorder::LittleEndian>(slots[k])
                .unwrap();
        }

        for k in 0..slots_size {
            dyn_writer
                .write_u64::<byteorder::LittleEndian>(amounts[k])
                .unwrap();
        }
    }
}

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
        let size = 137; // 32 + 32 + 1 + 8 + 4*8 + 4*8;
        let mut dst = vec![0; size];
        vesting_account.pack_into_slice(&mut dst);

        // Then I can unpack it from a slice.
        let va = VestingAccount::unpack_from_slice(&dst).unwrap();
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
