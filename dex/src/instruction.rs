use crate::error::DexError;
use crate::matching::{OrderType, Side};
use bytemuck::{bytes_of, cast};
#[cfg(test)]
use serde::{Deserialize, Serialize};
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};

use arrayref::{array_ref, array_refs};
use std::num::NonZeroU64;

#[cfg(test)]
use proptest::prelude::*;
#[cfg(test)]
use proptest_derive::Arbitrary;

pub mod srm_token {
    use solana_sdk::declare_id;
    declare_id!("SRMuApVNdxXokk5GT7XD5cUUgXMBCoAz2LHeuAoKWRt");
}

pub mod msrm_token {
    use solana_sdk::declare_id;
    declare_id!("MSRMcoVyrFxnSgo5uXwone5SKcGhT1KEJMFEkMEWf9L");
}

pub(crate) mod disable_authority {
    use solana_sdk::declare_id;
    declare_id!("5ZVJgwWxMsqXxRMYHXqMwH2hd4myX5Ef4Au2iUsuNQ7V");
}

pub(crate) mod fee_sweeper {
    use solana_sdk::declare_id;
    declare_id!("DeqYsmBd9BnrbgUwQjVH4sQWK71dEgE6eoZFw3Rp4ftE");
}

#[derive(PartialEq, Eq, Debug, Clone)]
#[cfg_attr(test, derive(Arbitrary, Serialize, Deserialize))]
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

#[derive(PartialEq, Eq, Debug, Clone)]
#[cfg_attr(test, derive(Arbitrary, Serialize, Deserialize))]
pub struct NewOrderInstruction {
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

#[derive(PartialEq, Eq, Debug, Clone)]
#[cfg_attr(test, derive(Arbitrary, Serialize, Deserialize))]
#[cfg_attr(feature = "fuzz", derive(arbitrary::Arbitrary))]
pub struct CancelOrderInstruction {
    pub side: Side,
    pub order_id: u128,
    pub owner: [u64; 4], // Unused
    pub owner_slot: u8,
}

#[derive(PartialEq, Eq, Debug, Clone)]
#[cfg_attr(test, derive(Arbitrary, Serialize, Deserialize))]
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
    NewOrder(NewOrderInstruction),
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
}

impl MarketInstruction {
    #[cfg(test)]
    #[inline]
    pub fn serde_pack(&self) -> Vec<u8> {
        bincode::serialize(&(0u8, self)).unwrap()
    }

    pub fn pack(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(43);
        match self {
            MarketInstruction::InitializeMarket(ref inst) => {
                buf.extend_from_slice(&[0; 5]);
                buf.extend_from_slice(&inst.coin_lot_size.to_le_bytes());
                buf.extend_from_slice(&inst.pc_lot_size.to_le_bytes());
                buf.extend_from_slice(&inst.fee_rate_bps.to_le_bytes());
                buf.extend_from_slice(&inst.vault_signer_nonce.to_le_bytes());
                buf.extend_from_slice(&inst.pc_dust_threshold.to_le_bytes());
            }
            MarketInstruction::NewOrder(ref inst) => {
                buf.extend_from_slice(&[0, 1, 0, 0, 0]);
                buf.extend_from_slice(&(inst.side as u32).to_le_bytes());
                buf.extend_from_slice(&inst.limit_price.get().to_le_bytes());
                buf.extend_from_slice(&inst.max_qty.get().to_le_bytes());
                buf.extend_from_slice(&(inst.order_type as u32).to_le_bytes());
                buf.extend_from_slice(&(inst.client_id).to_le_bytes());
            }
            MarketInstruction::MatchOrders(limit) => {
                buf.extend_from_slice(&[0, 2, 0, 0, 0]);
                buf.extend_from_slice(&limit.to_le_bytes());
            }
            MarketInstruction::ConsumeEvents(limit) => {
                buf.extend_from_slice(&[0, 3, 0, 0, 0]);
                buf.extend_from_slice(&limit.to_le_bytes());
            }
            MarketInstruction::CancelOrder(ref inst) => {
                buf.extend_from_slice(&[0, 4, 0, 0, 0]);
                buf.extend_from_slice(&(inst.side as u32).to_le_bytes());
                buf.extend_from_slice(&inst.order_id.to_le_bytes());
                buf.extend_from_slice(bytes_of(&inst.owner));
                buf.push(inst.owner_slot);
            }
            MarketInstruction::SettleFunds => {
                buf.extend_from_slice(&[0, 5, 0, 0, 0]);
            }
            MarketInstruction::CancelOrderByClientId(client_id) => {
                buf.extend_from_slice(&[0, 6, 0, 0, 0]);
                buf.extend_from_slice(&client_id.to_le_bytes());
            }
            MarketInstruction::DisableMarket => buf.extend_from_slice(&[0, 7, 0, 0, 0]),
            MarketInstruction::SweepFees => buf.extend_from_slice(&[0, 8, 0, 0, 0]),
        };
        buf
    }

    pub fn unpack(versioned_bytes: &[u8]) -> Option<Self> {
        if versioned_bytes.len() < 5 || versioned_bytes.len() > 58 {
            return None;
        }
        let (&[version], &discrim, data) = array_refs![versioned_bytes, 1, 4; ..;];
        if version != 0 {
            return None;
        }
        Some(match u32::from_le_bytes(discrim) {
            0 if data.len() == 34 => MarketInstruction::InitializeMarket({
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
            1 if (data.len() == 24 || data.len() == 32) => MarketInstruction::NewOrder({
                let (&side_arr, &price_arr, &max_qty_arr, &otype_arr, client_id_bytes) =
                    array_refs![data, 4, 8, 8, 4; .. ;];
                let client_id = match client_id_bytes.len() {
                    0 => 0,
                    8 => u64::from_le_bytes(*array_ref![client_id_bytes, 0, 8]),
                    _ => unreachable!(),
                };
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
                NewOrderInstruction {
                    side,
                    limit_price,
                    max_qty,
                    order_type,
                    client_id,
                }
            }),
            2 if data.len() == 2 => {
                let limit = array_ref![data, 0, 2];
                MarketInstruction::MatchOrders(u16::from_le_bytes(*limit))
            }
            3 if data.len() == 2 => {
                let limit = array_ref![data, 0, 2];
                MarketInstruction::ConsumeEvents(u16::from_le_bytes(*limit))
            }
            4 if data.len() == 53 => MarketInstruction::CancelOrder({
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
            5 => MarketInstruction::SettleFunds,
            6 if data.len() == 8 => {
                let client_id = array_ref![data, 0, 8];
                MarketInstruction::CancelOrderByClientId(u64::from_le_bytes(*client_id))
            }
            7 => MarketInstruction::DisableMarket,
            8 => MarketInstruction::SweepFees,
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
) -> Result<solana_sdk::instruction::Instruction, DexError> {
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
            let serialized = inst.serde_pack();
            let unpack_serde_result = MarketInstruction::unpack_serde(&serialized).ok();
            let unpack_result = MarketInstruction::unpack(&serialized);
            assert_eq!(unpack_result, Some(inst));
            assert!(unpack_serde_result == unpack_result,
                "Serialized:\n{:?}\nLeft:\n{:#?}\nRight:\n{:#?}",
                serialized, unpack_serde_result, unpack_result
            );
        }
    }
    proptest! {
        #[test]
        fn test_hand_serde_pack(inst: MarketInstruction) {
            let serde_packed = inst.serde_pack();
            let hand_packed = inst.pack();
            assert_eq!(serde_packed, hand_packed);
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
    }

    impl TryFrom<NewOrderInstructionU64> for NewOrderInstruction {
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

    impl From<&NewOrderInstruction> for NewOrderInstructionU64 {
        fn from(value: &NewOrderInstruction) -> Self {
            Self {
                side: value.side,
                limit_price: value.limit_price.get(),
                max_qty: value.max_qty.get(),
                order_type: value.order_type,
                client_id: value.client_id,
            }
        }
    }

    impl arbitrary::Arbitrary for NewOrderInstruction {
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
