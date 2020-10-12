use solana_client_gen::solana_sdk::pubkey::Pubkey;

pub fn signer_seeds<'a>(instance_account: &'a Pubkey, nonce: &'a u8) -> [&'a [u8]; 2] {
    [instance_account.as_ref(), bytemuck::bytes_of(nonce)]
}
