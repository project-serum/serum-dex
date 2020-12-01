use solana_client_gen::solana_sdk::pubkey::Pubkey;

pub fn signer_seeds<'a>(
    safe_account: &'a Pubkey,
    beneficiary: &'a Pubkey,
    nonce: &'a u8,
) -> [&'a [u8]; 3] {
    [
        safe_account.as_ref(),
        beneficiary.as_ref(),
        bytemuck::bytes_of(nonce),
    ]
}
