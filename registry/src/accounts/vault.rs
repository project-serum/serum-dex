use solana_client_gen::solana_sdk::pubkey::Pubkey;

/// A Vault is an SPL token account *owned* by a program-derived-address
/// defined by the Registry account instance and the nonce it was initialized
/// with.
pub fn signer_seeds<'a>(registry: &'a Pubkey, nonce: &'a u8) -> [&'a [u8]; 2] {
    [registry.as_ref(), bytemuck::bytes_of(nonce)]
}
