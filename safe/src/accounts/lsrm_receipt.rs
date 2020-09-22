use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use solana_client_gen::solana_sdk::program_error::ProgramError;
use solana_client_gen::solana_sdk::pubkey::Pubkey;
use spl_token::pack::{IsInitialized, Pack, Sealed};

/// LsrmReceipt is a program owned account. It's existence is a
/// proof of validity for an individual lSRM token.
pub struct LsrmReceipt {
    pub initialized: bool,
    /// The unique mint of the lSRM token.
    pub mint: Pubkey,
    /// The SPL token account associated with the mint.
    pub spl_account: Pubkey,
    /// The vesting_account this lSRM receipt was printed from.
    pub vesting_account: Pubkey,
    /// True if this receipt has been burned. Ensures each lSRM has one time
    /// use.
    pub burned: bool,
}

impl LsrmReceipt {
    pub const SIZE: usize = 98;
}

impl IsInitialized for LsrmReceipt {
    fn is_initialized(&self) -> bool {
        self.initialized
    }
}

impl Sealed for LsrmReceipt {}

impl Pack for LsrmReceipt {
    const LEN: usize = LsrmReceipt::SIZE;

    fn unpack_from_slice(src: &[u8]) -> Result<Self, ProgramError> {
        let src = array_ref![src, 0, LsrmReceipt::LEN];
        let (initialized, mint, spl_account, vesting_account, burned) =
            array_refs![src, 1, 32, 32, 32, 1];

        Ok(LsrmReceipt {
            initialized: match initialized {
                [0] => false,
                [1] => true,
                _ => return Err(ProgramError::InvalidAccountData),
            },
            mint: Pubkey::new(mint),
            spl_account: Pubkey::new(spl_account),
            vesting_account: Pubkey::new(vesting_account),
            burned: match burned {
                [0] => false,
                [1] => true,
                _ => return Err(ProgramError::InvalidAccountData),
            },
        })
    }

    fn pack_into_slice(&self, dst: &mut [u8]) {
        let dst = array_mut_ref![dst, 0, LsrmReceipt::LEN];
        let (initialized_dst, mint_dst, spl_account_dst, vesting_account_dst, burned_dst) =
            mut_array_refs![dst, 1, 32, 32, 32, 1];
        let LsrmReceipt {
            initialized,
            mint,
            spl_account,
            vesting_account,
            burned,
        } = self;
        initialized_dst[0] = *initialized as u8;
        mint_dst.copy_from_slice(mint.as_ref());
        spl_account_dst.copy_from_slice(spl_account.as_ref());
        vesting_account_dst.copy_from_slice(vesting_account.as_ref());
        burned_dst[0] = *burned as u8;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::OsRng;
    use solana_client_gen::solana_sdk::signature::{Keypair, Signer};

    #[test]
    fn lsrm_receipt_pack_unpack() {
        let mint = Keypair::generate(&mut OsRng).pubkey();
        let spl_account = Keypair::generate(&mut OsRng).pubkey();
        let vesting_account = Keypair::generate(&mut OsRng).pubkey();
        let receipt = LsrmReceipt {
            initialized: true,
            mint,
            spl_account,
            vesting_account,
            burned: true,
        };

        let mut dst = vec![0; LsrmReceipt::SIZE];
        receipt.pack_into_slice(&mut dst);
        let new_receipt = LsrmReceipt::unpack_from_slice(&dst).unwrap();

        assert_eq!(new_receipt.initialized, true);
        assert_eq!(new_receipt.mint, mint);
        assert_eq!(new_receipt.spl_account, spl_account);
        assert_eq!(new_receipt.vesting_account, vesting_account);
        assert_eq!(new_receipt.burned, true);
    }
}
