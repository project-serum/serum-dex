use borsh::{BorshDeserialize, BorshSchema, BorshSerialize};
use solana_sdk::pubkey::Pubkey;

#[derive(Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize, BorshSchema)]
pub struct Address([u8; 32]);

impl From<Address> for Pubkey {
    fn from(address: Address) -> Self {
        Pubkey::new_from_array(address.0)
    }
}

impl From<Pubkey> for Address {
    fn from(pubkey: Pubkey) -> Self {
        Self(pubkey.to_bytes())
    }
}

#[derive(Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize, BorshSchema)]
pub struct PoolState {
    pub pool_token_mint: Address,
    pub assets: Vec<AssetInfo>,

    /// Mint authority for the pool token and owner for the assets in the pool.
    pub vault_signer: Address,
    /// Nonce used to generate `vault_signer`.
    pub vault_signer_nonce: u8,

    /// Additional accounts that need to be included with every request.
    pub account_params: Vec<ParamDesc>,

    /// Meaning depends on the pool implementation.
    pub admin_key: Option<Address>,

    pub custom_state: Vec<u8>,
}

#[derive(Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize, BorshSchema)]
pub struct AssetInfo {
    pub mint: Address,
    /// Vault should be owned by `PoolState::vault_signer`
    pub vault_address: Address,
}

#[derive(Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize, BorshSchema)]
pub struct ParamDesc {
    pub address: Address,
    pub writable: bool,
}

#[derive(Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize, BorshSchema)]
pub struct Basket {
    /// Must have the same length as `PoolState::assets`. Each item corresponds to
    /// one of the assets in `PoolState::assets` and represents the quantity of
    /// that asset needed to create/redeem one pool token.
    pub qty_per_share: Vec<U64F64>,
}

#[derive(Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize, BorshSchema)]
pub struct U64F64 {
    pub frac_part: u64,
    pub int_part: u64,
}

#[derive(Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize, BorshSchema)]
pub struct Retbuf {
    pub retbuf_account: Address,
    pub retbuf_program_id: Address,
}

#[derive(Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize, BorshSchema)]
pub enum PoolRequest {
    // TODO
    GetInitializeParams,

    // TODO
    Initialize,

    /// Get the creation, redemption, or swap basket.
    ///
    /// Basket is written to the retbuf account as a Vec<i64>.
    ///
    /// Accounts:
    ///
    /// - `[writable]` Pool account
    /// - `[writable]` Pool token mint (`PoolState::pool_token_mint`)
    /// - `[writable]` Pool vault account for each of the N pool assets (`AssetInfo::vault_address`)
    /// - `[]` Pool vault authority (`PoolState::vault_signer`)
    /// - `[writable]` retbuf account
    /// - `[]` retbuf program
    /// - `[]/[writable]` Accounts in `PoolState::account_params`
    GetBasket(PoolAction),

    /// Perform a creation, redemption, or swap.
    ///
    /// Accounts:
    ///
    /// - `[writable]` Pool account
    /// - `[writable]` Pool token mint (`PoolState::pool_token_mint`)
    /// - `[writable]` Pool vault account for each of the N pool assets (`AssetInfo::vault_address`)
    /// - `[]` Pool vault authority (`PoolState::vault_signer`)
    /// - `[writable]` User pool token account
    /// - `[writable]` User account for each of the N pool assets
    /// - `[signer]` Authority for user accounts
    /// - `[]` spl-token program
    /// - `[]/[writable]` Accounts in `PoolState::account_params`
    Transact(PoolAction),

    // TODO
    AdminRequest,

    CustomRequest(Vec<u8>),
}

#[derive(Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize, BorshSchema)]
pub enum PoolAction {
    /// Create pool tokens by depositing assets into the pool.
    Create(u64),
    /// Redeem pool tokens by burning the token and receiving assets from the pool.
    Redeem(u64),
    /// Deposit assets into the pool and receive other assets from the pool.
    Swap(Vec<u64>),
}
