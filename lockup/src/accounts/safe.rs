use borsh::{BorshDeserialize, BorshSchema, BorshSerialize};
use serum_common::pack::*;
use solana_client_gen::prelude::*;

/// Safe is the account representing an instance of this program.
#[derive(Default, Debug, BorshSerialize, BorshDeserialize, BorshSchema)]
pub struct Safe {
    /// Is `true` if this structure has been initialized
    pub initialized: bool,
    /// The key with the ability to change the whitelist.
    pub authority: Pubkey,
    /// The whitelist of valid programs the Safe can relay transactions to.
    pub whitelist: Pubkey,
}

serum_common::packable!(Safe);

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::OsRng;
    use solana_client_gen::solana_sdk::program_error::ProgramError;
    use solana_client_gen::solana_sdk::signature::{Keypair, Signer};

    #[test]
    fn pack_unpack() {
        let authority = Keypair::generate(&mut OsRng).pubkey();
        let initialized = true;
        let whitelist = Pubkey::new_rand();
        let safe = Safe {
            authority: authority.clone(),
            initialized,
            whitelist,
        };

        let mut dst = Vec::new();
        dst.resize(Safe::default().size().unwrap() as usize, 0u8);

        Safe::pack(safe, &mut dst).unwrap();

        let new_safe = Safe::unpack(&dst).unwrap();

        assert_eq!(new_safe.authority, authority);
        assert_eq!(new_safe.initialized, initialized);
        assert_eq!(new_safe.whitelist, whitelist);
    }

    #[test]
    fn unpack_too_small() {
        let size = Safe::default().size().unwrap() - 10;
        let data = vec![0; size as usize];
        let result = Safe::unpack(&data);
        match result {
            Ok(_) => panic!("expect error"),
            Err(e) => assert_eq!(e, ProgramError::InvalidAccountData),
        }
    }

    #[test]
    fn unpack_too_large() {
        let size = Safe::default().size().unwrap() + 10;
        let data = vec![0; size as usize];
        let result = Safe::unpack(&data);
        match result {
            Ok(_) => panic!("expect error"),
            Err(e) => assert_eq!(e, ProgramError::InvalidAccountData),
        }
    }

    #[test]
    fn unpack_zeroes_size() {
        let og_size = Safe::default().size().unwrap();
        let zero_data = vec![0; og_size as usize];
        let r = Safe::unpack(&zero_data).unwrap();
        assert_eq!(r.initialized, false);
        assert_eq!(r.authority, Pubkey::new(&[0; 32]));
    }
}
