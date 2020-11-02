use solana_client_gen::solana_sdk::pubkey::Pubkey;

pub fn signer_seeds<'a>(registrar: &'a Pubkey, nonce: &'a u8) -> [&'a [u8]; 2] {
    [registrar.as_ref(), bytemuck::bytes_of(nonce)]
}
