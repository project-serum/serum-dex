use solana_client_gen::solana_sdk::pubkey::Pubkey;

/// Safe is the account representing an instance of this program (akin to an
/// SPL token's "mint").
#[derive(Default, Debug, serde::Serialize, serde::Deserialize)]
pub struct Safe {
    /// The mint of the SPL token the safe is storing, e.g., the SRM mint.
    pub mint: Pubkey,
    /// The key with the ability to migrate or change the authority.
    pub authority: Pubkey,
    /// Is `true` if this structure has been initialized
    pub initialized: bool,
    /// The nonce to use for the program-derived-address owning the Safe's
    /// token vault.
    pub nonce: u8,
}

serum_common::packable!(Safe);

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::OsRng;
    use solana_client_gen::solana_sdk::signature::{Keypair, Signer};

    #[test]
    fn safe_pack_unpack() {
        let mint = Keypair::generate(&mut OsRng).pubkey();
        let authority = Keypair::generate(&mut OsRng).pubkey();
        let initialized = true;
        let safe = Safe {
            mint: mint.clone(),
            authority: authority.clone(),
            initialized,
            nonce: 33,
        };

        let mut dst = Vec::new();
        dst.resize(Safe::default().size().unwrap() as usize, 0u8);
        Safe::pack(safe, &mut dst).unwrap();

        let new_safe = Safe::unpack(&dst).unwrap();

        assert_eq!(new_safe.mint, mint);
        assert_eq!(new_safe.authority, authority);
        assert_eq!(new_safe.initialized, initialized);
        assert_eq!(new_safe.nonce, 33);
    }
}
