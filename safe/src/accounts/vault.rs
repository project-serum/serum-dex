use solana_client_gen::solana_sdk::pubkey::Pubkey;

pub struct SrmVault;
impl SrmVault {
    pub fn signer_seeds<'a>(safe_account: &'a Pubkey, nonce: &'a [u8]) -> [&'a [u8]; 2] {
        [safe_account.as_ref(), nonce]
    }
}
