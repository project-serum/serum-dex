use crate::pack::DynPack;
use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use byteorder::{ReadBytesExt, WriteBytesExt};
use solana_client_gen::solana_sdk::program_error::ProgramError;
use solana_client_gen::solana_sdk::pubkey::Pubkey;
// TODO: this is in the solana_sdk. Use that version instead.
use spl_token::pack::{IsInitialized, Pack, Sealed};

/// SafeAccount is the account representing an instance of the SrmSafe,
/// akin to SPL's "mint".
#[repr(C)]
#[derive(Debug)]
pub struct SafeAccount {
    /// The mint of the SPL token the safe is storing, i.e., the SRM mint.
    pub mint: Pubkey,
    /// Optional authority used to mint new tokens.
    pub authority: Pubkey,
    /// Total SRM deposited.
    pub supply: u64,
    /// Is `true` if this structure has been initialized
    pub is_initialized: bool,
    /// The list of acceptable program ids to send lSRM to.
    pub whitelist: Whitelist,
}

impl SafeAccount {
    pub const SIZE: usize = 393;
}

impl IsInitialized for SafeAccount {
    fn is_initialized(&self) -> bool {
        self.is_initialized
    }
}

impl Sealed for SafeAccount {}

impl Pack for SafeAccount {
    const LEN: usize = SafeAccount::SIZE;

    fn unpack_from_slice(src: &[u8]) -> Result<Self, ProgramError> {
        let src = array_ref![src, 0, SafeAccount::LEN];
        let (mint, authority, supply, is_initialized, whitelist) =
            array_refs![src, 32, 32, 8, 1, Whitelist::SIZE];

        Ok(SafeAccount {
            mint: Pubkey::new(mint),
            authority: Pubkey::new(authority),
            supply: u64::from_le_bytes(*supply),
            is_initialized: match is_initialized {
                [0] => false,
                [1] => true,
                _ => return Err(ProgramError::InvalidAccountData),
            },
            whitelist: Whitelist::from_bytes(whitelist),
        })
    }

    fn pack_into_slice(&self, dst: &mut [u8]) {
        let dst = array_mut_ref![dst, 0, SafeAccount::LEN];
        let (mint_dst, authority_dst, supply_dst, is_initialized_dst, whitelist_dst) =
            mut_array_refs![dst, 32, 32, 8, 1, Whitelist::SIZE];
        let SafeAccount {
            mint,
            authority,
            supply,
            is_initialized,
            whitelist,
        } = self;
        mint_dst.copy_from_slice(mint.as_ref());
        authority_dst.copy_from_slice(authority.as_ref());
        *supply_dst = supply.to_le_bytes();
        is_initialized_dst[0] = *is_initialized as u8;
        whitelist.to_bytes(whitelist_dst);
    }
}

// TODO: decide on this number. 10 is arbitrary.
//
// TODO: use a macro so we don't have to manually expand eveerything here.
#[repr(C)]
#[derive(Clone, Debug, PartialEq)]
pub struct Whitelist([Pubkey; 10]);
impl Whitelist {
    pub const SIZE: usize = 320;

    pub fn new(inner: [Pubkey; 10]) -> Self {
        Self(inner)
    }

    pub fn zeroed() -> Self {
        Self([
            Pubkey::new_from_array([0; 32]),
            Pubkey::new_from_array([0; 32]),
            Pubkey::new_from_array([0; 32]),
            Pubkey::new_from_array([0; 32]),
            Pubkey::new_from_array([0; 32]),
            Pubkey::new_from_array([0; 32]),
            Pubkey::new_from_array([0; 32]),
            Pubkey::new_from_array([0; 32]),
            Pubkey::new_from_array([0; 32]),
            Pubkey::new_from_array([0; 32]),
        ])
    }

    pub fn get_at(&self, index: usize) -> &Pubkey {
        &self.0[index]
    }

    pub fn add_at(&mut self, index: usize, pk: Pubkey) {
        self.0[index] = pk;
    }

    pub fn push(&mut self, pk: Pubkey) -> Option<usize> {
        let mut idx = None;
        for (k, pk) in self.0.iter().enumerate() {
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
        for (k, pk) in self.0.iter().enumerate() {
            if *pk == pk_remove {
                idx = Some(k);
                break;
            }
        }

        idx.map(|idx| {
            self.0[idx] = Pubkey::new_from_array([0; 32]);
            idx
        })
    }

    pub fn from_bytes(bytes: &[u8]) -> Self {
        let mut whitelist = Whitelist::zeroed();
        for k in 0..10 {
            let start = 32 * k;
            let end = start + 32;
            let pid = Pubkey::new(&bytes[start..end]);
            whitelist.add_at(k, pid);
        }
        whitelist
    }

    pub fn to_bytes(&self, dst: &mut [u8]) {
        let dst = array_mut_ref![dst, 0, Whitelist::SIZE];
        let (zero, one, two, three, four, five, six, seven, eight, nine) =
            mut_array_refs![dst, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32];
        zero.copy_from_slice(self.get_at(0).as_ref());
        one.copy_from_slice(self.get_at(1).as_ref());
        two.copy_from_slice(self.get_at(2).as_ref());
        three.copy_from_slice(self.get_at(3).as_ref());
        four.copy_from_slice(self.get_at(4).as_ref());
        five.copy_from_slice(self.get_at(5).as_ref());
        six.copy_from_slice(self.get_at(6).as_ref());
        seven.copy_from_slice(self.get_at(7).as_ref());
        eight.copy_from_slice(self.get_at(8).as_ref());
        nine.copy_from_slice(self.get_at(9).as_ref());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::OsRng;
    use solana_client_gen::solana_sdk::signature::{Keypair, Signer};

    #[test]
    fn safe_account_pack_unpack() {
        let mint = Keypair::generate(&mut OsRng).pubkey();
        let authority = Keypair::generate(&mut OsRng).pubkey();
        let supply = 123;
        let is_initialized = true;
        let whitelist = Whitelist::new([
            Keypair::generate(&mut OsRng).pubkey(),
            Keypair::generate(&mut OsRng).pubkey(),
            Keypair::generate(&mut OsRng).pubkey(),
            Keypair::generate(&mut OsRng).pubkey(),
            Keypair::generate(&mut OsRng).pubkey(),
            Keypair::generate(&mut OsRng).pubkey(),
            Keypair::generate(&mut OsRng).pubkey(),
            Keypair::generate(&mut OsRng).pubkey(),
            Keypair::generate(&mut OsRng).pubkey(),
            Keypair::generate(&mut OsRng).pubkey(),
        ]);
        let safe = SafeAccount {
            mint: mint.clone(),
            authority: authority.clone(),
            supply,
            is_initialized,
            whitelist: whitelist.clone(),
        };

        let mut dst = vec![0; SafeAccount::SIZE];
        safe.pack_into_slice(&mut dst);

        let new_safe = SafeAccount::unpack_from_slice(&dst).unwrap();

        assert_eq!(new_safe.mint, mint);
        assert_eq!(new_safe.authority, authority);
        assert_eq!(new_safe.supply, supply);
        assert_eq!(new_safe.is_initialized, is_initialized);
        assert_eq!(new_safe.whitelist, whitelist);
    }
}
