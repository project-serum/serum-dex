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

    /// Indicative only. May be out-of-date for dynamic pools.
    pub creation_basket: Basket,
    /// Indicative only. May be out-of-date for dynamic pools.
    pub redemption_basket: Basket,

    /// Mint authority for the pool token and owner for the assets in the pool.
    pub vault_signer: Address,
    /// Nonce used to generate `vault_signer`.
    pub vault_signer_nonce: u8,

    /// Addresses that need to be included with every request.
    pub address_params: Vec<ParamDesc>,

    /// Meaning depends on the pool implementation.
    pub admin_key: Option<Address>,
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
pub struct PoolRequest {
    pub state: Address,
    pub retbuf: Retbuf,

    pub inner: PoolRequestInner,
}

#[derive(Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize, BorshSchema)]
pub enum PoolRequestInner {
    RefreshBasket,
    Creation(CreationRequest),
    Redemption(RedemptionRequest),
    InitPool(PoolState),
    Admin {
        admin_signature: Address,
        admin_request: AdminRequest,
    },
}

#[derive(Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize, BorshSchema)]
pub struct CreationRequestInput {
    pub token_account: Address,
    pub signer: Address,
    pub max_qty_per_share: U64F64,
}

#[derive(Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize, BorshSchema)]
pub struct RedemptionRequestOutput {
    pub token_account: Address,
    pub signer: Address,
    pub min_qty_per_share: U64F64,
}

#[derive(Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize, BorshSchema)]
pub struct CreationRequest {
    pub inputs: Vec<CreationRequestInput>,
    pub output_to: Address,
}

#[derive(Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize, BorshSchema)]
pub struct RedemptionRequest {
    pub pool_token_account: Address,
    pub pool_token_signer: Address,
    pub outputs: Vec<RedemptionRequestOutput>,
}

#[derive(Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize, BorshSchema)]
pub enum AdminRequest {
    SetPendingAdmin(Address),
    // SetBasket(PoolBasket),
}
