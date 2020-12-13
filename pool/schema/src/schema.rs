use std::collections::HashMap;
use std::{io, io::Write};

use borsh::schema::{Declaration, Definition};
use borsh::{BorshDeserialize, BorshSchema, BorshSerialize};
use solana_program::pubkey::Pubkey;

/// Wrapper around `solana_sdk::pubkey::Pubkey` so it can implement `BorshSerialize` etc.
#[repr(transparent)]
#[derive(Clone, PartialEq, Eq)]
pub struct Address(Pubkey);

impl From<Address> for Pubkey {
    fn from(address: Address) -> Self {
        address.0
    }
}

impl AsRef<Pubkey> for Address {
    fn as_ref(&self) -> &Pubkey {
        &self.0
    }
}

impl AsMut<Pubkey> for Address {
    fn as_mut(&mut self) -> &mut Pubkey {
        &mut self.0
    }
}

impl From<Pubkey> for Address {
    fn from(pubkey: Pubkey) -> Self {
        Self(pubkey)
    }
}

impl From<&Pubkey> for Address {
    fn from(pubkey: &Pubkey) -> Self {
        Self(*pubkey)
    }
}

#[macro_export]
macro_rules! declare_tag {
    ($name:ident, $type:ty, $tag:expr) => {
        #[derive(Clone, PartialEq, Eq, BorshSerialize, BorshSchema, Debug)]
        pub struct $name($type);
        impl $name {
            pub const TAG_VALUE: $type = $tag;
        }

        impl Default for $name {
            fn default() -> Self {
                Self(Self::TAG_VALUE)
            }
        }

        impl BorshDeserialize for $name {
            #[inline]
            fn deserialize(buf: &mut &[u8]) -> std::io::Result<Self> {
                let tag = <$type as BorshDeserialize>::deserialize(buf)?;
                if tag != Self::TAG_VALUE {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "invalid tag",
                    ));
                }
                Ok($name(tag))
            }
        }
    };
}

pub mod fee_owner {
    use solana_program::declare_id;

    declare_id!("3LTvJCPiPSMjX1kBJ6ZfEhn4G2hM46aJ1yEZsk8i12TK");
}

declare_tag!(PoolStateTag, u64, 0x16a7874c7fb2301b);

#[derive(Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize, BorshSchema)]
pub struct PoolState {
    pub tag: PoolStateTag,

    /// Token mint account for the pool token.
    pub pool_token_mint: Address,
    /// Mint and vaults for the assets in the pool.
    pub assets: Vec<AssetInfo>,

    /// Mint authority for the pool token and owner for the assets in the pool.
    pub vault_signer: Address,
    /// Nonce used to generate `vault_signer`.
    pub vault_signer_nonce: u8,

    /// Additional accounts that need to be included with every request.
    pub account_params: Vec<ParamDesc>,

    /// User-friendly pool name.
    pub name: String,

    /// Vault for fees collected by the pool for LQD. Mint is the pool token mint.
    pub lqd_fee_vault: Address,
    /// Vault for fees collected by the pool for the pool initializer. Mint is the pool token mint.
    pub initializer_fee_vault: Address,

    /// Fee on creations and redemptions, per million tokens.
    pub fee_rate: u32,

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

pub const MIN_FEE_RATE: u32 = 150;
pub const DEFAULT_FEE_RATE: u32 = 2500;
pub const FEE_RATE_DENOMINATOR: u32 = 1_000_000;

declare_tag!(PoolRequestTag, u64, 0x220a6cbdcd1cc4cf);

#[derive(Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize, BorshSchema)]
pub struct PoolRequest {
    pub tag: PoolRequestTag,
    pub inner: PoolRequestInner,
}

#[derive(Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize, BorshSchema)]
pub enum PoolRequestInner {
    /// Initialize a pool.
    ///
    /// Accounts:
    ///
    /// - `[writable]` Pool account
    /// - `[writable]` Pool token mint (`PoolState::pool_token_mint`)
    /// - `[writable]` Pool vault account for each of the N pool assets (`AssetInfo::vault_address`)
    /// - `[]` Pool vault authority (`PoolState::vault_signer`)
    /// - `[]` LQD fee vault
    /// - `[]` Initializer fee vault
    /// - `[]` Rent sysvar
    /// - `[]/[writable]` Any additional accounts needed to initialize the pool
    Initialize(InitializePoolRequest),

    /// Get the creation, redemption, or swap basket.
    ///
    /// Basket is written to the retbuf account as a Vec<i64>.
    ///
    /// Accounts:
    ///
    /// - `[]` Pool account
    /// - `[]` Pool token mint (`PoolState::pool_token_mint`)
    /// - `[]` Pool vault account for each of the N pool assets (`AssetInfo::vault_address`)
    /// - `[]` Pool vault authority (`PoolState::vault_signer`)
    /// - `[writable]` retbuf account
    /// - `[]` retbuf program
    /// - `[]` Accounts in `PoolState::account_params`
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
    /// - `[writable]` LQD fee vault
    /// - `[writable]` Initializer fee vault
    /// - `[writable]` Referrer fee vault
    /// - `[]` spl-token program
    /// - `[]/[writable]` Accounts in `PoolState::account_params`
    Execute(PoolAction),
}

#[derive(Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize, BorshSchema)]
pub struct InitializePoolRequest {
    pub vault_signer_nonce: u8,
    pub assets_length: u8,
    pub pool_name: String,
    pub fee_rate: u32,
    pub custom_data: Vec<u8>,
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

#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize, BorshSchema)]
pub struct Basket {
    /// Must have the same length as `PoolState::assets`. Each item corresponds to
    /// one of the assets in `PoolState::assets`.
    pub quantities: Vec<i64>,
}

impl BorshSerialize for Address {
    fn serialize<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        BorshSerialize::serialize(&self.0.to_bytes(), writer)
    }
}

impl BorshDeserialize for Address {
    fn deserialize(buf: &mut &[u8]) -> io::Result<Self> {
        Ok(Self(Pubkey::new_from_array(BorshDeserialize::deserialize(
            buf,
        )?)))
    }
}

impl BorshSchema for Address {
    fn add_definitions_recursively(definitions: &mut HashMap<Declaration, Definition>) {
        Self::add_definition(
            Self::declaration(),
            Definition::Struct {
                fields: borsh::schema::Fields::UnnamedFields(vec![
                    <[u8; 32] as BorshSchema>::declaration(),
                ]),
            },
            definitions,
        );
        <[u8; 32] as BorshSchema>::add_definitions_recursively(definitions);
    }

    fn declaration() -> Declaration {
        "Address".to_string()
    }
}
