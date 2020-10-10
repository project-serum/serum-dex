use arrayref::{array_mut_ref, mut_array_refs};
use serde::{Deserialize, Serialize};
use serum_common::pack::*;
use solana_client_gen::solana_sdk::pubkey::Pubkey;
use std::io::Read;

pub const SIZE: usize = 320;

// TODO: decide on this number. 10 is arbitrary.
//
// TODO: use a macro so we don't have to manually expand eveerything here.
#[derive(Default, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Whitelist {
    pub programs: [Pubkey; 10],
}

impl Whitelist {
    pub fn new(programs: [Pubkey; 10]) -> Self {
        Self { programs }
    }

    pub fn get_at(&self, index: usize) -> &Pubkey {
        &self.programs[index]
    }

    pub fn add_at(&mut self, index: usize, pk: Pubkey) {
        self.programs[index] = pk;
    }

    pub fn push(&mut self, pk: Pubkey) -> Option<usize> {
        let mut idx = None;
        for (k, pk) in self.programs.iter().enumerate() {
            if *pk == Pubkey::new_from_array([0; 32]) {
                idx = Some(k);
                break;
            }
        }
        idx.map(|idx| {
            self.add_at(idx, pk);
            idx
        })
    }

    pub fn delete(&mut self, pk_remove: Pubkey) -> Option<usize> {
        let mut idx = None;
        for (k, pk) in self.programs.iter().enumerate() {
            if *pk == pk_remove {
                idx = Some(k);
                break;
            }
        }

        idx.map(|idx| {
            self.programs[idx] = Pubkey::new_from_array([0; 32]);
            idx
        })
    }

    pub fn contains(&self, p: &Pubkey) -> bool {
        for pk in self.programs.iter() {
            if pk == p {
                return true;
            }
        }
        false
    }
}

impl Pack for Whitelist {
    fn unpack_unchecked(src: &mut &[u8]) -> Result<Whitelist, ProgramError> {
        // TODO: don't read to the end of this array.
        let mut new_src = vec![];
        src.read_to_end(&mut new_src)
            .map_err(|_| ProgramError::InvalidAccountData)?;

        let mut whitelist = Whitelist::default();

        for k in 0..10 {
            let start = 32 * k;
            let end = start + 32;
            let pid = Pubkey::new(&new_src[start..end]);
            whitelist.add_at(k, pid);
        }

        Ok(whitelist)
    }

    fn pack(src: Whitelist, dst: &mut [u8]) -> Result<(), ProgramError> {
        let dst = array_mut_ref![dst, 0, SIZE];
        let (zero, one, two, three, four, five, six, seven, eight, nine) =
            mut_array_refs![dst, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32];

        zero.copy_from_slice(src.get_at(0).as_ref());
        one.copy_from_slice(src.get_at(1).as_ref());
        two.copy_from_slice(src.get_at(2).as_ref());
        three.copy_from_slice(src.get_at(3).as_ref());
        four.copy_from_slice(src.get_at(4).as_ref());
        five.copy_from_slice(src.get_at(5).as_ref());
        six.copy_from_slice(src.get_at(6).as_ref());
        seven.copy_from_slice(src.get_at(7).as_ref());
        eight.copy_from_slice(src.get_at(8).as_ref());
        nine.copy_from_slice(src.get_at(9).as_ref());

        Ok(())
    }

    fn size(&self) -> Result<u64, ProgramError> {
        Ok(SIZE as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_unpack() {
        let whitelist = Whitelist {
            programs: [
                Pubkey::new_rand(),
                Pubkey::new_rand(),
                Pubkey::new_rand(),
                Pubkey::new_rand(),
                Pubkey::new_rand(),
                Pubkey::new_rand(),
                Pubkey::new_rand(),
                Pubkey::new_rand(),
                Pubkey::new_rand(),
                Pubkey::new_rand(),
            ],
        };

        let mut dst = Vec::new();
        dst.resize(Whitelist::default().size().unwrap() as usize, 0u8);

        Whitelist::pack(whitelist.clone(), &mut dst).unwrap();

        let wl = Whitelist::unpack(&dst).unwrap();

        assert_eq!(wl, whitelist);
    }
}
