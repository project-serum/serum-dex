//! mod accounts defines the storage layout for the accounts used by this program.

use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use solana_client_gen::solana_sdk::program_error::ProgramError;
use solana_client_gen::solana_sdk::pubkey::Pubkey;
use spl_token::pack::{IsInitialized, Pack, Sealed};

/// SafeAccount is the account representing an instance of the SrmSafe,
/// akin to SPL's "mint".
#[repr(C)]
#[derive(Debug)]
pub struct SafeAccount {
    /// Optional authority used to mint new tokens.
    pub authority: Pubkey,
    /// Total SRM deposited.
    pub supply: u64,
    /// Is `true` if this structure has been initialized
    pub is_initialized: bool,
}

impl SafeAccount {
    pub const SIZE: usize = 41;
}

impl Sealed for SafeAccount {}

impl Pack for SafeAccount {
    const LEN: usize = SafeAccount::SIZE;

    fn unpack_from_slice(src: &[u8]) -> Result<Self, ProgramError> {
        let src = array_ref![src, 0, SafeAccount::LEN];
        let (authority, supply, is_initialized) = array_refs![src, 32, 8, 1];

        Ok(SafeAccount {
            authority: Pubkey::new(authority),
            supply: u64::from_le_bytes(*supply),
            is_initialized: match is_initialized {
                [0] => false,
                [1] => true,
                _ => return Err(ProgramError::InvalidAccountData),
            },
        })
    }
    fn pack_into_slice(&self, dst: &mut [u8]) {
        let dst = array_mut_ref![dst, 0, SafeAccount::LEN];
        let (authority_dst, supply_dst, is_initialized_dst) = mut_array_refs![dst, 32, 8, 1];
        let &SafeAccount {
            authority,
            supply,
            is_initialized,
        } = self;
        authority_dst.copy_from_slice(authority.as_ref());
        *supply_dst = supply.to_le_bytes();
        is_initialized_dst[0] = is_initialized as u8;
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct VestingAccount {}

impl VestingAccount {}
