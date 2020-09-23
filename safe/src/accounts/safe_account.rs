use solana_client_gen::solana_sdk::pubkey::Pubkey;

/// SafeAccount is the account representing an instance of the SrmSafe,
/// akin to SPL's "mint".
#[derive(Default, Debug, serde::Serialize, serde::Deserialize)]
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
    pub initialized: bool,
    /// The nonce to use for the vault signer.
    pub nonce: u8,
}

serum_common::packable!(SafeAccount);

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
        let initialized = true;
        let safe = SafeAccount {
            mint: mint.clone(),
            authority: authority.clone(),
            supply,
            initialized,
            nonce: 33,
        };

        let mut dst = Vec::new();
        dst.resize(SafeAccount::size().unwrap() as usize, 0u8);
        SafeAccount::pack(safe, &mut dst).unwrap();

        let new_safe = SafeAccount::unpack(&dst).unwrap();

        assert_eq!(new_safe.mint, mint);
        assert_eq!(new_safe.authority, authority);
        assert_eq!(new_safe.supply, supply);
        assert_eq!(new_safe.initialized, initialized);
        assert_eq!(new_safe.nonce, 33);
    }
}
