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
    pub basket: PoolBasket,
    pub admin_key: Option<Address>,
    pub pool_token: PoolTokenInfo,
}

#[derive(Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize, BorshSchema)]
pub struct ParamDesc {
    pub address: Address,
    pub writable: bool,
}

#[derive(Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize, BorshSchema)]
pub struct U64F64 {
    pub frac_part: u64,
    pub int_part: u64,
}

#[derive(Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize, BorshSchema)]
pub struct BasketComponent {
    pub token_mint: Address,
    pub qty_per_share: U64F64,
    pub vault_address: Address,
    pub vault_signer_nonce: u8,
}

#[derive(Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize, BorshSchema)]
pub struct SimpleBasket {
    pub components: Vec<BasketComponent>,
}

type RequiredParams = Vec<ParamDesc>;

#[derive(Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize, BorshSchema)]
pub struct DelegationPolicy {
    pub delegate_program: Address,
    pub creation_params: RequiredParams,
    pub redemption_params: RequiredParams,
    pub refresh_basket_params: RequiredParams,
    pub only_delegate_when_empty: bool,
}

#[derive(Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize, BorshSchema)]
pub struct DynamicBasket {
    pub creation_basket: SimpleBasket,
    pub redemption_basket: SimpleBasket,

    pub delegation_policy: DelegationPolicy,
}

#[derive(Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize, BorshSchema)]
pub enum PoolBasket {
    Simple(SimpleBasket),
    Dynamic(DynamicBasket),
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
pub struct PoolTokenInfo {
    pub mint_address: Address,
    pub vault_address: Address,
    pub vault_signer_nonce: u8,
}

#[derive(Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize, BorshSchema)]
pub enum AdminRequest {
    SetPendingAdmin(Address),
    SetBasket(PoolBasket),
}
