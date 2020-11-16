#![cfg_attr(not(feature = "program"), allow(unused))]
use crate::error::DexError;
use crate::matching::{OrderType, Side};
use bytemuck::cast;
use serde::{Deserialize, Serialize};
use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use std::convert::TryInto;

use arrayref::{array_ref, array_refs};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use std::num::NonZeroU64;

#[cfg(test)]
use proptest::prelude::*;
#[cfg(test)]
use proptest_derive::Arbitrary;

pub mod srm_token {
    use solana_program::declare_id;
    declare_id!("SRMuApVNdxXokk5GT7XD5cUUgXMBCoAz2LHeuAoKWRt");
}

pub mod msrm_token {
    use solana_program::declare_id;
    declare_id!("MSRMcoVyrFxnSgo5uXwone5SKcGhT1KEJMFEkMEWf9L");
}

pub mod disable_authority {
    use solana_program::declare_id;
    declare_id!("5ZVJgwWxMsqXxRMYHXqMwH2hd4myX5Ef4Au2iUsuNQ7V");
}

pub mod fee_sweeper {
    use solana_program::declare_id;
    declare_id!("DeqYsmBd9BnrbgUwQjVH4sQWK71dEgE6eoZFw3Rp4ftE");
}

#[derive(PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(test, derive(Arbitrary))]
#[cfg_attr(test, proptest(no_params))]
#[cfg_attr(feature = "fuzz", derive(arbitrary::Arbitrary))]
pub struct InitializeMarketInstruction {
    // In the matching engine, all prices and balances are integers.
    // This only works if the smallest representable quantity of the coin
    // is at least a few orders of magnitude larger than the smallest representable
    // quantity of the price currency. The internal representation also relies on
    // on the assumption that every order will have a (quantity x price) value that
    // fits into a u64.
    //
    // If these assumptions are problematic, rejigger the lot sizes.
    pub coin_lot_size: u64,
    pub pc_lot_size: u64,
    pub fee_rate_bps: u16,
    pub vault_signer_nonce: u64,
    pub pc_dust_threshold: u64,
}

#[derive(
    PartialEq, Eq, Copy, Clone, Debug, TryFromPrimitive, IntoPrimitive, Serialize, Deserialize,
)]
#[cfg_attr(test, derive(Arbitrary))]
#[cfg_attr(feature = "fuzz", derive(arbitrary::Arbitrary))]
#[repr(u8)]
pub enum SelfTradeBehavior {
    DecrementTake = 0,
    CancelProvide = 1,
}

#[derive(PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(test, derive(Arbitrary))]
pub struct NewOrderInstructionV2 {
    pub side: Side,
    #[cfg_attr(
        test,
        proptest(strategy = "(1u64..=std::u64::MAX).prop_map(|x| NonZeroU64::new(x).unwrap())")
    )]
    pub limit_price: NonZeroU64,
    #[cfg_attr(
        test,
        proptest(strategy = "(1u64..=std::u64::MAX).prop_map(|x| NonZeroU64::new(x).unwrap())")
    )]
    pub max_qty: NonZeroU64,
    pub order_type: OrderType,
    pub client_id: u64,
    pub self_trade_behavior: SelfTradeBehavior,
}

#[derive(PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(test, derive(Arbitrary))]
pub struct NewOrderInstructionV1 {
    pub side: Side,
    #[cfg_attr(
        test,
        proptest(strategy = "(1u64..=std::u64::MAX).prop_map(|x| NonZeroU64::new(x).unwrap())")
    )]
    pub limit_price: NonZeroU64,
    #[cfg_attr(
        test,
        proptest(strategy = "(1u64..=std::u64::MAX).prop_map(|x| NonZeroU64::new(x).unwrap())")
    )]
    pub max_qty: NonZeroU64,
    pub order_type: OrderType,
    pub client_id: u64,
}

impl NewOrderInstructionV1 {
    pub fn add_self_trade_behavior(
        self,
        self_trade_behavior: SelfTradeBehavior,
    ) -> NewOrderInstructionV2 {
        let NewOrderInstructionV1 {
            side,
            limit_price,
            max_qty,
            order_type,
            client_id,
        } = self;
        NewOrderInstructionV2 {
            side,
            limit_price,
            max_qty,
            order_type,
            client_id,
            self_trade_behavior,
        }
    }
}

impl NewOrderInstructionV1 {
    fn unpack(data: &[u8; 32]) -> Option<Self> {
        let (&side_arr, &price_arr, &max_qty_arr, &otype_arr, &client_id_bytes) =
            array_refs![data, 4, 8, 8, 4, 8];
        let client_id = u64::from_le_bytes(client_id_bytes);
        let side = match u32::from_le_bytes(side_arr) {
            0 => Side::Bid,
            1 => Side::Ask,
            _ => return None,
        };
        let limit_price = NonZeroU64::new(u64::from_le_bytes(price_arr))?;
        let max_qty = NonZeroU64::new(u64::from_le_bytes(max_qty_arr))?;
        let order_type = match u32::from_le_bytes(otype_arr) {
            0 => OrderType::Limit,
            1 => OrderType::ImmediateOrCancel,
            2 => OrderType::PostOnly,
            _ => return None,
        };
        Some(NewOrderInstructionV1 {
            side,
            limit_price,
            max_qty,
            order_type,
            client_id,
        })
    }
}

#[derive(PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(test, derive(Arbitrary))]
#[cfg_attr(feature = "fuzz", derive(arbitrary::Arbitrary))]
pub struct CancelOrderInstruction {
    pub side: Side,
    pub order_id: u128,
    pub owner: [u64; 4], // Unused
    pub owner_slot: u8,
}

#[derive(PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(test, derive(Arbitrary))]
#[cfg_attr(feature = "fuzz", derive(arbitrary::Arbitrary))]
pub enum MarketInstruction {
    /// 0. `[writable]` the market to initialize
    /// 1. `[writable]` zeroed out request queue
    /// 2. `[writable]` zeroed out event queue
    /// 3. `[writable]` zeroed out bids
    /// 4. `[writable]` zeroed out asks
    /// 5. `[writable]` spl-token account for the coin currency
    /// 6. `[writable]` spl-token account for the price currency
    /// 7. `[]` coin currency Mint
    /// 8. `[]` price currency Mint
    InitializeMarket(InitializeMarketInstruction),
    /// 0. `[writable]` the market
    /// 1. `[writable]` the OpenOrders account to use
    /// 2. `[writable]` the request queue
    /// 3. `[writable]` the (coin or price currency) account paying for the order
    /// 4. `[signer]` owner of the OpenOrders account
    /// 5. `[writable]` coin vault
    /// 6. `[writable]` pc vault
    /// 7. `[]` spl token program
    /// 8. `[]` the rent sysvar
    /// 9. `[writable]` (optional) the (M)SRM account used for fee discounts
    NewOrder(NewOrderInstructionV1),
    /// 0. `[writable]` market
    /// 1. `[writable]` req_q
    /// 2. `[writable]` event_q
    /// 3. `[writable]` bids
    /// 4. `[writable]` asks
    /// 5. `[writable]` coin fee receivable account
    /// 6. `[writable]` pc fee receivable account
    MatchOrders(u16),
    /// ... `[writable]` OpenOrders
    /// accounts.len() - 4 `[writable]` market
    /// accounts.len() - 3 `[writable]` event queue
    /// accounts.len() - 2 `[writable]` coin fee receivable account
    /// accounts.len() - 1 `[writable]` pc fee receivable account
    ConsumeEvents(u16),
    /// 0. `[]` market
    /// 1. `[writable]` OpenOrders
    /// 2. `[writable]` the request queue
    /// 3. `[signer]` the OpenOrders owner
    CancelOrder(CancelOrderInstruction),
    /// 0. `[writable]` market
    /// 1. `[writable]` OpenOrders
    /// 2. `[signer]` the OpenOrders owner
    /// 3. `[writable]` coin vault
    /// 4. `[writable]` pc vault
    /// 5. `[writable]` coin wallet
    /// 6. `[writable]` pc wallet
    /// 7. `[]` vault signer
    /// 8. `[]` spl token program
    /// 9. `[writable]` (optional) referrer pc wallet
    SettleFunds,
    /// 0. `[]` market
    /// 1. `[writable]` OpenOrders
    /// 2. `[writable]` the request queue
    /// 3. `[signer]` the OpenOrders owner
    CancelOrderByClientId(u64),
    /// 0. `[writable]` market
    /// 1. `[signer]` disable authority
    DisableMarket,
    /// 0. `[writable]` market
    /// 1. `[writable]` pc vault
    /// 2. `[signer]` fee sweeping authority
    /// 3. `[writable]` fee receivable account
    /// 4. `[]` vault signer
    /// 5. `[]` spl token program
    SweepFees,
    /// 0. `[writable]` the market
    /// 1. `[writable]` the OpenOrders account to use
    /// 2. `[writable]` the request queue
    /// 3. `[writable]` the (coin or price currency) account paying for the order
    /// 4. `[signer]` owner of the OpenOrders account
    /// 5. `[writable]` coin vault
    /// 6. `[writable]` pc vault
    /// 7. `[]` spl token program
    /// 8. `[]` the rent sysvar
    /// 9. `[writable]` (optional) the (M)SRM account used for fee discounts
    NewOrderV2(NewOrderInstructionV2),
}

impl MarketInstruction {
    pub fn pack(&self) -> Vec<u8> {
        bincode::serialize(&(0u8, self)).unwrap()
    }

    pub fn unpack(versioned_bytes: &[u8]) -> Option<Self> {
        if versioned_bytes.len() < 5 || versioned_bytes.len() > 58 {
            return None;
        }
        let (&[version], &discrim, data) = array_refs![versioned_bytes, 1, 4; ..;];
        if version != 0 {
            return None;
        }
        let discrim = u32::from_le_bytes(discrim);
        Some(match (discrim, data.len()) {
            (0, 34) => MarketInstruction::InitializeMarket({
                let data_array = array_ref![data, 0, 34];
                let fields = array_refs![data_array, 8, 8, 2, 8, 8];
                InitializeMarketInstruction {
                    coin_lot_size: u64::from_le_bytes(*fields.0),
                    pc_lot_size: u64::from_le_bytes(*fields.1),
                    fee_rate_bps: u16::from_le_bytes(*fields.2),
                    vault_signer_nonce: u64::from_le_bytes(*fields.3),
                    pc_dust_threshold: u64::from_le_bytes(*fields.4),
                }
            }),
            (1, 32) => MarketInstruction::NewOrder({
                let data_arr = array_ref![data, 0, 32];
                NewOrderInstructionV1::unpack(data_arr)?
            }),
            (2, 2) => {
                let limit = array_ref![data, 0, 2];
                MarketInstruction::MatchOrders(u16::from_le_bytes(*limit))
            }
            (3, 2) => {
                let limit = array_ref![data, 0, 2];
                MarketInstruction::ConsumeEvents(u16::from_le_bytes(*limit))
            }
            (4, 53) => MarketInstruction::CancelOrder({
                let data_array = array_ref![data, 0, 53];
                let fields = array_refs![data_array, 4, 16, 32, 1];
                let side = match u32::from_le_bytes(*fields.0) {
                    0 => Side::Bid,
                    1 => Side::Ask,
                    _ => return None,
                };
                let order_id = u128::from_le_bytes(*fields.1);
                let owner = cast(*fields.2);
                let &[owner_slot] = fields.3;
                CancelOrderInstruction {
                    side,
                    order_id,
                    owner,
                    owner_slot,
                }
            }),
            (5, 0) => MarketInstruction::SettleFunds,
            (6, 8) => {
                let client_id = array_ref![data, 0, 8];
                MarketInstruction::CancelOrderByClientId(u64::from_le_bytes(*client_id))
            }
            (7, 0) => MarketInstruction::DisableMarket,
            (8, 0) => MarketInstruction::SweepFees,
            (9, 36) => MarketInstruction::NewOrderV2({
                let data_arr = array_ref![data, 0, 36];
                let (v1_data_arr, v2_data_arr) = array_refs![data_arr, 32, 4];
                let v1_instr = NewOrderInstructionV1::unpack(v1_data_arr)?;
                let self_trade_behavior = SelfTradeBehavior::try_from_primitive(
                    u32::from_le_bytes(*v2_data_arr).try_into().ok()?,
                )
                .ok()?;
                v1_instr.add_self_trade_behavior(self_trade_behavior)
            }),
            _ => return None,
        })
    }

    #[cfg(test)]
    #[inline]
    pub fn unpack_serde(data: &[u8]) -> Result<Self, ()> {
        match data.split_first() {
            None => Err(()),
            Some((&0u8, rest)) => bincode::deserialize(rest).map_err(|_| ()),
            Some((_, _rest)) => Err(()),
        }
    }
}

pub fn initialize_market(
    market: &Pubkey,
    program_id: &Pubkey,
    coin_mint_pk: &Pubkey,
    pc_mint_pk: &Pubkey,
    coin_vault_pk: &Pubkey,
    pc_vault_pk: &Pubkey,
    // srm_vault_pk: &Pubkey,
    bids_pk: &Pubkey,
    asks_pk: &Pubkey,
    req_q_pk: &Pubkey,
    event_q_pk: &Pubkey,
    coin_lot_size: u64,
    pc_lot_size: u64,
    vault_signer_nonce: u64,
    pc_dust_threshold: u64,
) -> Result<solana_program::instruction::Instruction, DexError> {
    let data = MarketInstruction::InitializeMarket(InitializeMarketInstruction {
        coin_lot_size,
        pc_lot_size,
        fee_rate_bps: 0,
        vault_signer_nonce,
        pc_dust_threshold,
    })
    .pack();

    let market_account = AccountMeta::new(*market, false);

    let bids = AccountMeta::new(*bids_pk, false);
    let asks = AccountMeta::new(*asks_pk, false);
    let req_q = AccountMeta::new(*req_q_pk, false);
    let event_q = AccountMeta::new(*event_q_pk, false);

    let coin_vault = AccountMeta::new(*coin_vault_pk, false);
    let pc_vault = AccountMeta::new(*pc_vault_pk, false);

    let coin_mint = AccountMeta::new_readonly(*coin_mint_pk, false);
    let pc_mint = AccountMeta::new_readonly(*pc_mint_pk, false);

    let accounts = vec![
        market_account,
        req_q,
        event_q,
        bids,
        asks,
        coin_vault,
        pc_vault,
        //srm_vault,
        coin_mint,
        pc_mint,
        //srm_mint,
    ];

    Ok(Instruction {
        program_id: *program_id,
        data,
        accounts,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    proptest! {
        #[test]
        fn test_pack_unpack_roundtrip(inst: MarketInstruction) {
            let serialized = inst.pack();
            let unpack_serde_result = MarketInstruction::unpack_serde(&serialized).ok();
            let unpack_result = MarketInstruction::unpack(&serialized);
            assert_eq!(unpack_result, Some(inst));
            assert!(unpack_serde_result == unpack_result,
                "Serialized:\n{:?}\nLeft:\n{:#?}\nRight:\n{:#?}",
                serialized, unpack_serde_result, unpack_result
            );
        }
    }
}

#[cfg(feature = "fuzz")]
mod fuzzing {
    use super::*;
    use crate::matching::{OrderType, Side};
    use arbitrary::Unstructured;
    use std::convert::{TryFrom, TryInto};

    #[derive(arbitrary::Arbitrary)]
    struct NewOrderInstructionU64 {
        pub side: Side,
        pub limit_price: u64,
        pub max_qty: u64,
        pub order_type: OrderType,
        pub client_id: u64,
        pub self_trade_behavior: SelfTradeBehavior,
    }

    impl TryFrom<NewOrderInstructionU64> for NewOrderInstructionV2 {
        type Error = std::num::TryFromIntError;

        fn try_from(value: NewOrderInstructionU64) -> Result<Self, Self::Error> {
            Ok(Self {
                side: value.side,
                limit_price: value.limit_price.try_into()?,
                max_qty: value.max_qty.try_into()?,
                order_type: value.order_type,
                client_id: value.client_id,
                self_trade_behavior: value.self_trade_behavior,
            })
        }
    }

    impl TryFrom<NewOrderInstructionU64> for NewOrderInstructionV1 {
        type Error = std::num::TryFromIntError;

        fn try_from(value: NewOrderInstructionU64) -> Result<Self, Self::Error> {
            Ok(Self {
                side: value.side,
                limit_price: value.limit_price.try_into()?,
                max_qty: value.max_qty.try_into()?,
                order_type: value.order_type,
                client_id: value.client_id,
            })
        }
    }

    impl From<&NewOrderInstructionV1> for NewOrderInstructionU64 {
        fn from(value: &NewOrderInstructionV1) -> Self {
            Self {
                side: value.side,
                limit_price: value.limit_price.get(),
                max_qty: value.max_qty.get(),
                order_type: value.order_type,
                client_id: value.client_id,
                self_trade_behavior: SelfTradeBehavior::DecrementTake,
            }
        }
    }

    impl From<&NewOrderInstructionV2> for NewOrderInstructionU64 {
        fn from(value: &NewOrderInstructionV2) -> Self {
            Self {
                side: value.side,
                limit_price: value.limit_price.get(),
                max_qty: value.max_qty.get(),
                order_type: value.order_type,
                client_id: value.client_id,
                self_trade_behavior: value.self_trade_behavior,
            }
        }
    }

    impl arbitrary::Arbitrary for NewOrderInstructionV1 {
        fn arbitrary(u: &mut Unstructured<'_>) -> Result<Self, arbitrary::Error> {
            <NewOrderInstructionU64 as arbitrary::Arbitrary>::arbitrary(u)?
                .try_into()
                .map_err(|_| arbitrary::Error::IncorrectFormat)
        }

        fn size_hint(depth: usize) -> (usize, Option<usize>) {
            <NewOrderInstructionU64 as arbitrary::Arbitrary>::size_hint(depth)
        }

        fn shrink(&self) -> Box<dyn Iterator<Item = Self>> {
            let x: NewOrderInstructionU64 = self.into();
            Box::new(
                x.shrink()
                    .map(NewOrderInstructionU64::try_into)
                    .filter_map(Result::ok),
            )
        }
    }

    impl arbitrary::Arbitrary for NewOrderInstructionV2 {
        fn arbitrary(u: &mut Unstructured<'_>) -> Result<Self, arbitrary::Error> {
            <NewOrderInstructionU64 as arbitrary::Arbitrary>::arbitrary(u)?
                .try_into()
                .map_err(|_| arbitrary::Error::IncorrectFormat)
        }

        fn size_hint(depth: usize) -> (usize, Option<usize>) {
            <NewOrderInstructionU64 as arbitrary::Arbitrary>::size_hint(depth)
        }

        fn shrink(&self) -> Box<dyn Iterator<Item = Self>> {
            let x: NewOrderInstructionU64 = self.into();
            Box::new(
                x.shrink()
                    .map(NewOrderInstructionU64::try_into)
                    .filter_map(Result::ok),
            )
        }
    }
}
