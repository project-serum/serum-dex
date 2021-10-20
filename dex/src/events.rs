use crate::state::EventView;
use crate::state::ToAlignedBytes;
use crate::{fees::FeeTier, matching::Side};
use anchor_lang::prelude::*;
use bytemuck::cast;
use std::num::NonZeroU64;

#[event]
pub struct FillEvent {
    pub side: Side,
    pub maker: bool,
    pub native_qty_paid: u64,
    pub native_qty_received: u64,
    pub native_fee_or_rebate: u64,
    pub order_id: u128,
    pub owner: Pubkey,
    pub owner_slot: u8,
    pub fee_tier: FeeTier,
    pub client_order_id: Option<u64>,
    #[index]
    pub timestamp: i64,
}

impl EventView {
    pub fn to_fill_event(self) -> Result<FillEvent, ProgramError> {
        match self {
            EventView::Fill {
                side,
                maker,
                native_qty_paid,
                native_qty_received,
                native_fee_or_rebate,
                order_id,
                owner,
                owner_slot,
                fee_tier,
                client_order_id,
            } => Ok(FillEvent {
                side,
                maker,
                native_qty_paid,
                native_qty_received,
                native_fee_or_rebate,
                order_id,
                owner: Pubkey::new_from_array(cast(owner)),
                owner_slot,
                fee_tier,
                client_order_id: client_order_id.map(|id| id.into()),
                timestamp: Clock::get()?.unix_timestamp,
            }),

            _ => Err(ProgramError::InvalidAccountData),
        }
    }
}
