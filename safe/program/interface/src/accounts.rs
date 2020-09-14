use solana_client_gen::solana_sdk::pubkey::Pubkey;

#[repr(C)]
#[derive(Debug)]
pub struct VestingAccount {}

impl VestingAccount {
    const SIZE: usize = 0;
}

/// SafeAccount is the account representing an instance of the SrmSafe,
/// akin to SPL's "mint".
#[repr(C)]
#[derive(Debug)]
pub struct SafeAccount {
    /// Optional authority used to mint new tokens.
    pub authority: Pubkey,
    /// Total SRM deposited.
    pub supply: u64,
    /// Is `true` if this structure has been initialized
    pub is_initialized: bool,
}

impl SafeAccount {
    pub const SIZE: usize = 41;
}
