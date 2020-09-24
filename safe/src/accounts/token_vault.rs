use solana_client_gen::solana_sdk::pubkey::Pubkey;

/// A TokenVault is just an SPL token account *owned* by a program-derived-address
/// defined by the Safe account's address and the nonce it was initialized
/// with.
pub struct TokenVault;
impl TokenVault {
    pub fn signer_seeds<'a>(safe_account: &'a Pubkey, nonce: &'a u8) -> [&'a [u8]; 2] {
        [safe_account.as_ref(), bytemuck::bytes_of(nonce)]
    }
}
