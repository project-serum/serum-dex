use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
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
    /// The key with the ability to migrate or change the authority.
    pub authority: Pubkey,
    /// Total SRM deposited.
    // TODO: we don't actually use this field right now, but it might
    //       be nice to have for quick queries.
    pub supply: u64,
    /// Is `true` if this structure has been initialized
    pub is_initialized: bool,
    /// The nonce to use for the vault signer.
    pub nonce: u8,
}

impl SafeAccount {
    pub const SIZE: usize = 74;
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
        let (mint, authority, supply, is_initialized, nonce) = array_refs![src, 32, 32, 8, 1, 1];

        Ok(SafeAccount {
            mint: Pubkey::new(mint),
            authority: Pubkey::new(authority),
            supply: u64::from_le_bytes(*supply),
            is_initialized: match is_initialized {
                [0] => false,
                [1] => true,
                _ => return Err(ProgramError::InvalidAccountData),
            },
            nonce: nonce[0],
        })
    }

    fn pack_into_slice(&self, dst: &mut [u8]) {
        let dst = array_mut_ref![dst, 0, SafeAccount::LEN];
        let (mint_dst, authority_dst, supply_dst, is_initialized_dst, nonce_dst) =
            mut_array_refs![dst, 32, 32, 8, 1, 1];
        let SafeAccount {
            mint,
            authority,
            supply,
            is_initialized,
            nonce,
        } = self;
        mint_dst.copy_from_slice(mint.as_ref());
        authority_dst.copy_from_slice(authority.as_ref());
        *supply_dst = supply.to_le_bytes();
        is_initialized_dst[0] = *is_initialized as u8;
        nonce_dst[0] = *nonce;
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
        let safe = SafeAccount {
            mint: mint.clone(),
            authority: authority.clone(),
            supply,
            is_initialized,
            nonce: 33,
        };

        let mut dst = vec![0; SafeAccount::SIZE];
        safe.pack_into_slice(&mut dst);

        let new_safe = SafeAccount::unpack_from_slice(&dst).unwrap();

        assert_eq!(new_safe.mint, mint);
        assert_eq!(new_safe.authority, authority);
        assert_eq!(new_safe.supply, supply);
        assert_eq!(new_safe.is_initialized, is_initialized);
        assert_eq!(new_safe.nonce, 33);
    }
}
