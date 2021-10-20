use crate::state::EventView;
use crate::{fees::FeeTier, matching::Side};
use anchor_lang::prelude::*;
use bytemuck::cast;

#[event]
pub struct FillEvent {
    pub side: Side,
    pub maker: bool,
    pub native_qty_paid: u64,
    pub native_qty_received: u64,
    pub native_fee_or_rebate: u64,
    pub order_id: u128,
    #[index]
    pub owner: Pubkey,
    pub owner_slot: u8,
    pub fee_tier: FeeTier,
    pub client_order_id: Option<u64>,
    #[index]
    pub timestamp: i64,
}

#[event]
pub struct OutEvent {
    pub side: Side,
    pub release_funds: bool,
    pub native_qty_unlocked: u64,
    pub native_qty_still_locked: u64,
    pub order_id: u128,
    #[index]
    pub owner: Pubkey,
    pub owner_slot: u8,
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

    pub fn to_out_event(self) -> Result<OutEvent, ProgramError> {
        match self {
            EventView::Out {
                side,
                release_funds,
                native_qty_unlocked,
                native_qty_still_locked,
                order_id,
                owner,
                owner_slot,
                client_order_id,
            } => Ok(OutEvent {
                side,
                release_funds,
                native_qty_unlocked,
                native_qty_still_locked,
                order_id,
                owner: Pubkey::new_from_array(cast(owner)),
                owner_slot,
                client_order_id: client_order_id.map(|id| id.into()),
                timestamp: Clock::get()?.unix_timestamp,
            }),
            _ => Err(ProgramError::InvalidAccountData),
        }
    }
}
