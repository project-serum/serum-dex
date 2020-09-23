use solana_client_gen::solana_sdk::pubkey::Pubkey;

/// LsrmReceipt is a program owned account. It's existence is a
/// proof of validity for an individual lSRM token.
#[derive(Default, Debug, serde::Serialize, serde::Deserialize)]
pub struct LsrmReceipt {
    /// True iff the receipt has been authorized by the program.
    pub initialized: bool,
    /// The unique mint of the lSRM token.
    pub mint: Pubkey,
    /// The SPL token account associated with the mint.
    pub spl_acc: Pubkey,
    /// The vesting_account this lSRM receipt was printed from.
    pub vesting_acc: Pubkey,
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
        let spl_acc = Keypair::generate(&mut OsRng).pubkey();
        let vesting_acc = Keypair::generate(&mut OsRng).pubkey();
        let receipt = LsrmReceipt {
            initialized: true,
            mint,
            spl_acc,
            vesting_acc,
            burned: true,
        };

        let mut dst = Vec::new();
        dst.resize(LsrmReceipt::default().size().unwrap() as usize, 0u8);
        LsrmReceipt::pack(receipt, &mut dst).unwrap();
        let new_receipt = LsrmReceipt::unpack(&dst).unwrap();

        assert_eq!(new_receipt.initialized, true);
        assert_eq!(new_receipt.mint, mint);
        assert_eq!(new_receipt.spl_acc, spl_acc);
        assert_eq!(new_receipt.vesting_acc, vesting_acc);
        assert_eq!(new_receipt.burned, true);
    }
}
