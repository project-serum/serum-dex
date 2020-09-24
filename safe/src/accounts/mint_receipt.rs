use solana_client_gen::solana_sdk::pubkey::Pubkey;

/// MintReceipt is a program owned account. It's existence is a proof of
/// validity for an individual NFT token backed by a Safe instance.
#[derive(Default, Debug, serde::Serialize, serde::Deserialize)]
pub struct MintReceipt {
    /// True iff the receipt has been authorized by the program.
    pub initialized: bool,
    /// The unique mint of the lSRM token.
    pub mint: Pubkey,
    /// The SPL token account associated with the mint.
    pub token_acc: Pubkey,
    /// The Safe vesting account this receipt was printed from.
    pub vesting_acc: Pubkey,
    /// True if this receipt has been burned. Ensures each MintReceipt
    /// has a one time use for redemption.
    pub burned: bool,
}

serum_common::packable!(MintReceipt);

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::OsRng;
    use solana_client_gen::solana_sdk::signature::{Keypair, Signer};

    #[test]
    fn lsrm_receipt_pack_unpack() {
        let mint = Keypair::generate(&mut OsRng).pubkey();
        let token_acc = Keypair::generate(&mut OsRng).pubkey();
        let vesting_acc = Keypair::generate(&mut OsRng).pubkey();
        let receipt = MintReceipt {
            initialized: true,
            mint,
            token_acc,
            vesting_acc,
            burned: true,
        };

        let mut dst = Vec::new();
        dst.resize(MintReceipt::default().size().unwrap() as usize, 0u8);
        MintReceipt::pack(receipt, &mut dst).unwrap();
        let new_receipt = MintReceipt::unpack(&dst).unwrap();

        assert_eq!(new_receipt.initialized, true);
        assert_eq!(new_receipt.mint, mint);
        assert_eq!(new_receipt.token_acc, token_acc);
        assert_eq!(new_receipt.vesting_acc, vesting_acc);
        assert_eq!(new_receipt.burned, true);
    }
}
