use solana_client_gen::solana_sdk::pubkey::Pubkey;

/// LsrmReceipt is a program owned account. It's existence is a
/// proof of validity for an individual lSRM token.
#[derive(Default, serde::Serialize, serde::Deserialize)]
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

serum_common::packable!(LsrmReceipt);

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

        let mut dst = Vec::new();
        dst.resize(LsrmReceipt::size().unwrap() as usize, 0u8);
        LsrmReceipt::pack(receipt, &mut dst).unwrap();
        let new_receipt = LsrmReceipt::unpack(&dst).unwrap();

        assert_eq!(new_receipt.initialized, true);
        assert_eq!(new_receipt.mint, mint);
        assert_eq!(new_receipt.spl_account, spl_account);
        assert_eq!(new_receipt.vesting_account, vesting_account);
        assert_eq!(new_receipt.burned, true);
    }
}
