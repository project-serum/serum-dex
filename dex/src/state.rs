#![cfg_attr(not(feature = "program"), allow(unused))]
use num_enum::TryFromPrimitive;
use std::{cell::RefMut, convert::TryInto, mem::size_of, num::NonZeroU64, ops::DerefMut};

use arrayref::{array_ref, array_refs, mut_array_refs};

use bytemuck::{
    bytes_of, bytes_of_mut, cast, cast_slice, cast_slice_mut, from_bytes_mut, try_cast_mut,
    try_cast_slice_mut, try_from_bytes_mut, Pod, Zeroable,
};
use enumflags2::BitFlags;
use num_traits::FromPrimitive;
use safe_transmute::{self, to_bytes::transmute_to_bytes, trivial::TriviallyTransmutable};

#[cfg(feature = "program")]
use solana_sdk::info;

#[cfg(not(feature = "program"))]
macro_rules! info {
    ($($i:expr),*) => { { ($($i),*) } };
}

use solana_sdk::{
    account_info::AccountInfo,
    program_error::ProgramError,
    program_pack::Pack,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::{Sysvar, SysvarId},
};
use spl_token::error::TokenError;

use crate::{
    critbit::Slab,
    error::{DexErrorCode, DexResult, SourceFileId},
    fees::{self, FeeTier},
    instruction::{
        disable_authority, fee_sweeper, msrm_token, srm_token, CancelOrderInstruction,
        InitializeMarketInstruction, MarketInstruction, NewOrderInstructionV2, SelfTradeBehavior,
    },
    matching::{OrderBookState, OrderType, Side},
};

declare_check_assert_macros!(SourceFileId::State);

pub trait ToAlignedBytes {
    fn to_aligned_bytes(&self) -> [u64; 4];
}

impl ToAlignedBytes for Pubkey {
    #[inline]
    fn to_aligned_bytes(&self) -> [u64; 4] {
        cast(self.to_bytes())
    }
}

#[derive(Copy, Clone, BitFlags, Debug, Eq, PartialEq)]
#[repr(u64)]
pub enum AccountFlag {
    Initialized = 1u64 << 0,
    Market = 1u64 << 1,
    OpenOrders = 1u64 << 2,
    RequestQueue = 1u64 << 3,
    EventQueue = 1u64 << 4,
    Bids = 1u64 << 5,
    Asks = 1u64 << 6,
    Disabled = 1u64 << 7,
}

#[cfg_attr(target_endian = "little", derive(Debug))]
#[derive(Copy, Clone)]
#[repr(C)]
pub struct MarketState {
    // 0
    pub account_flags: u64, // Initialized, Market

    // 1
    pub own_address: [u64; 4],

    // 5
    pub vault_signer_nonce: u64,
    // 6
    pub coin_mint: [u64; 4],
    // 10
    pub pc_mint: [u64; 4],

    // 14
    pub coin_vault: [u64; 4],
    // 18
    pub coin_deposits_total: u64,
    // 19
    pub coin_fees_accrued: u64,

    // 20
    pub pc_vault: [u64; 4],
    // 24
    pub pc_deposits_total: u64,
    // 25
    pub pc_fees_accrued: u64,

    // 26
    pub pc_dust_threshold: u64,

    // 27
    pub req_q: [u64; 4],
    // 31
    pub event_q: [u64; 4],

    // 35
    pub bids: [u64; 4],
    // 39
    pub asks: [u64; 4],

    // 43
    pub coin_lot_size: u64,
    // 44
    pub pc_lot_size: u64,

    // 45
    pub fee_rate_bps: u64,
    // 46
    pub referrer_rebates_accrued: u64,
}
#[cfg(target_endian = "little")]
unsafe impl Zeroable for MarketState {}
#[cfg(target_endian = "little")]
unsafe impl Pod for MarketState {}
#[cfg(target_endian = "little")]
unsafe impl TriviallyTransmutable for MarketState {}

pub const ACCOUNT_HEAD_PADDING: &[u8; 5] = b"serum";
pub const ACCOUNT_TAIL_PADDING: &[u8; 7] = b"padding";

fn init_account_padding(data: &mut [u8]) -> DexResult<&mut [u64]> {
    check_assert!(data.len() >= 12)?;
    let (head, data, tail) = mut_array_refs![data, 5; ..; 7];
    *head = *ACCOUNT_HEAD_PADDING;
    *tail = *ACCOUNT_TAIL_PADDING;
    Ok(try_cast_slice_mut(data).or(check_unreachable!())?)
}

fn check_account_padding(data: &mut [u8]) -> DexResult<&mut [u64]> {
    check_assert!(data.len() >= 12)?;
    let (head, data, tail) = mut_array_refs![data, 5; ..; 7];
    check_assert_eq!(head, ACCOUNT_HEAD_PADDING)?;
    check_assert_eq!(tail, ACCOUNT_TAIL_PADDING)?;
    Ok(try_cast_slice_mut(data).or(check_unreachable!())?)
}

fn strip_account_padding(padded_data: &mut [u8], init_allowed: bool) -> DexResult<&mut [u64]> {
    if init_allowed {
        init_account_padding(padded_data)
    } else {
        check_account_padding(padded_data)
    }
}

pub fn strip_header<'a, H: Pod, D: Pod>(
    account: &'a AccountInfo,
    init_allowed: bool,
) -> DexResult<(RefMut<'a, H>, RefMut<'a, [D]>)> {
    let mut result = Ok(());
    let (header, inner): (RefMut<'a, [H]>, RefMut<'a, [D]>) =
        RefMut::map_split(account.try_borrow_mut_data()?, |padded_data| {
            let dummy_value: (&mut [H], &mut [D]) = (&mut [], &mut []);
            let padded_data: &mut [u8] = *padded_data;
            let u64_data = match strip_account_padding(padded_data, init_allowed) {
                Ok(u64_data) => u64_data,
                Err(e) => {
                    result = Err(e);
                    return dummy_value;
                }
            };

            let data: &mut [u8] = cast_slice_mut(u64_data);
            let (header_bytes, inner_bytes) = data.split_at_mut(size_of::<H>());
            let header: &mut H;
            let inner: &mut [D];

            header = match try_from_bytes_mut(header_bytes) {
                Ok(h) => h,
                Err(_e) => {
                    result = Err(assertion_error!().into());
                    return dummy_value;
                }
            };
            inner = remove_slop_mut(inner_bytes);

            (std::slice::from_mut(header), inner)
        });
    result?;
    let header = RefMut::map(header, |s| s.first_mut().unwrap_or_else(|| unreachable!()));
    Ok((header, inner))
}

impl MarketState {
    #[inline]
    pub fn load<'a>(
        market_account: &'a AccountInfo,
        program_id: &Pubkey,
    ) -> DexResult<RefMut<'a, Self>> {
        check_assert_eq!(market_account.owner, program_id)?;
        let mut account_data: RefMut<'a, [u8]>;
        let state: RefMut<'a, Self>;

        account_data = RefMut::map(market_account.try_borrow_mut_data()?, |data| *data);
        check_account_padding(&mut account_data)?;
        state = RefMut::map(account_data, |data| {
            from_bytes_mut(cast_slice_mut(
                check_account_padding(data).unwrap_or_else(|_| unreachable!()),
            ))
        });

        state.check_flags()?;
        Ok(state)
    }

    #[inline]
    pub fn check_flags(&self) -> DexResult {
        let flags = BitFlags::from_bits(self.account_flags)
            .map_err(|_| DexErrorCode::InvalidMarketFlags)?;
        let required_flags = AccountFlag::Initialized | AccountFlag::Market;
        if flags != required_flags {
            Err(DexErrorCode::InvalidMarketFlags)?
        }
        Ok(())
    }

    pub fn load_orders_mut<'a>(
        &self,
        orders_account: &'a AccountInfo,
        owner_account: Option<&AccountInfo>,
        program_id: &Pubkey,
        rent: Option<Rent>,
    ) -> DexResult<RefMut<'a, OpenOrders>> {
        check_assert_eq!(orders_account.owner, program_id)?;
        let mut open_orders: RefMut<'a, OpenOrders>;

        let open_orders_data_len = orders_account.data_len();
        let open_orders_lamports = orders_account.lamports();
        let (_, data) = strip_header::<[u8; 0], u8>(orders_account, true)?;
        open_orders = RefMut::map(data, |data| from_bytes_mut(data));

        if open_orders.account_flags == 0 {
            let rent = rent.ok_or(DexErrorCode::RentNotProvided)?;
            let owner_account = owner_account.ok_or(DexErrorCode::OwnerAccountNotProvided)?;
            if !rent.is_exempt(open_orders_lamports, open_orders_data_len) {
                return Err(DexErrorCode::OrdersNotRentExempt)?;
            }
            open_orders.init(&self.own_address, &owner_account.key.to_aligned_bytes())?;
        }
        open_orders.check_flags()?;
        check_assert_eq!(&open_orders.market, &self.own_address)
            .map_err(|_| DexErrorCode::WrongOrdersAccount)?;
        if let Some(owner) = owner_account {
            check_assert_eq!(&open_orders.owner, &owner.key.to_aligned_bytes())
                .map_err(|_| DexErrorCode::WrongOrdersAccount)?;
        }

        Ok(open_orders)
    }

    fn load_bids_mut<'a>(&self, bids: &'a AccountInfo) -> DexResult<RefMut<'a, Slab>> {
        check_assert_eq!(&bids.key.to_aligned_bytes(), &self.bids)
            .map_err(|_| DexErrorCode::WrongBidsAccount)?;
        let (header, buf) = strip_header::<OrderBookStateHeader, u8>(bids, false)?;
        let flags = BitFlags::from_bits(header.account_flags).unwrap();
        check_assert_eq!(&flags, &(AccountFlag::Initialized | AccountFlag::Bids))?;
        Ok(RefMut::map(buf, Slab::new))
    }

    fn load_asks_mut<'a>(&self, asks: &'a AccountInfo) -> DexResult<RefMut<'a, Slab>> {
        check_assert_eq!(&asks.key.to_aligned_bytes(), &self.asks)
            .map_err(|_| DexErrorCode::WrongAsksAccount)?;
        let (header, buf) = strip_header::<OrderBookStateHeader, u8>(asks, false)?;
        let flags = BitFlags::from_bits(header.account_flags).unwrap();
        check_assert_eq!(&flags, &(AccountFlag::Initialized | AccountFlag::Asks))?;
        Ok(RefMut::map(buf, Slab::new))
    }

    fn load_request_queue_mut<'a>(&self, queue: &'a AccountInfo) -> DexResult<RequestQueue<'a>> {
        check_assert_eq!(&queue.key.to_aligned_bytes(), &self.req_q)
            .map_err(|_| DexErrorCode::WrongRequestQueueAccount)?;

        let (header, buf) = strip_header::<RequestQueueHeader, Request>(queue, false)?;
        let flags = BitFlags::from_bits(header.account_flags).unwrap();
        check_assert_eq!(
            &flags,
            &(AccountFlag::Initialized | AccountFlag::RequestQueue)
        )?;
        Ok(Queue { header, buf })
    }

    fn load_event_queue_mut<'a>(&self, queue: &'a AccountInfo) -> DexResult<EventQueue<'a>> {
        check_assert_eq!(&queue.key.to_aligned_bytes(), &self.event_q)
            .map_err(|_| DexErrorCode::WrongEventQueueAccount)?;
        let (header, buf) = strip_header::<EventQueueHeader, Event>(queue, false)?;

        let flags = BitFlags::from_bits(header.account_flags).unwrap();
        check_assert_eq!(
            &flags,
            &(AccountFlag::Initialized | AccountFlag::EventQueue)
        )?;
        Ok(Queue { header, buf })
    }

    #[inline]
    fn check_coin_vault(&self, vault: account_parser::TokenAccount) -> DexResult {
        if self.coin_vault != vault.inner().key.to_aligned_bytes() {
            Err(DexErrorCode::WrongCoinVault)?
        }
        Ok(())
    }

    #[inline]
    fn check_pc_vault(&self, vault: account_parser::TokenAccount) -> DexResult {
        if self.pc_vault != vault.inner().key.to_aligned_bytes() {
            Err(DexErrorCode::WrongPcVault)?
        }
        Ok(())
    }

    #[inline]
    fn check_coin_payer(&self, payer: account_parser::TokenAccount) -> DexResult {
        if &payer.inner().try_borrow_data()?[..32] != transmute_to_bytes(&self.coin_mint) {
            Err(DexErrorCode::WrongCoinMint)?
        }
        Ok(())
    }

    #[inline]
    fn check_pc_payer(&self, payer: account_parser::TokenAccount) -> DexResult {
        if &payer.inner().try_borrow_data()?[..32] != transmute_to_bytes(&self.pc_mint) {
            Err(DexErrorCode::WrongPcMint)?
        }
        Ok(())
    }

    #[inline]
    fn load_fee_tier(
        &self,
        expected_owner: &[u64; 4],
        srm_or_msrm_account: Option<account_parser::TokenAccount>,
    ) -> DexResult<FeeTier> {
        let srm_or_msrm_account = match srm_or_msrm_account {
            Some(a) => a,
            None => return Ok(FeeTier::Base),
        };
        let data = srm_or_msrm_account.inner().try_borrow_data()?;

        let mut aligned_data: [u64; 9] = Zeroable::zeroed();
        bytes_of_mut(&mut aligned_data).copy_from_slice(&data[..72]);
        let (mint, owner, &[balance]) = array_refs![&aligned_data, 4, 4, 1];

        check_assert_eq!(owner, expected_owner)?;
        if mint == &srm_token::ID.to_aligned_bytes() {
            return Ok(FeeTier::from_srm_and_msrm_balances(balance, 0));
        }

        if mint == &msrm_token::ID.to_aligned_bytes() {
            return Ok(FeeTier::from_srm_and_msrm_balances(0, balance));
        }

        Ok(FeeTier::from_srm_and_msrm_balances(0, 0))
    }

    fn check_enabled(&self) -> DexResult {
        let flags = BitFlags::from_bits(self.account_flags).unwrap();
        if flags.contains(AccountFlag::Disabled) {
            return Err(DexErrorCode::MarketIsDisabled.into());
        }
        Ok(())
    }

    fn pubkey(&self) -> Pubkey {
        Pubkey::new(cast_slice(&self.own_address as &[_]))
    }
}

#[cfg_attr(feature = "fuzz", derive(Debug))]
#[repr(C)]
#[derive(Copy, Clone)]
pub struct OpenOrders {
    pub account_flags: u64, // Initialized, OpenOrders
    pub market: [u64; 4],
    pub owner: [u64; 4],

    pub native_coin_free: u64,
    pub native_coin_total: u64,

    pub native_pc_free: u64,
    pub native_pc_total: u64,

    pub free_slot_bits: u128,
    pub is_bid_bits: u128,
    pub orders: [u128; 128],
    // Using Option<NonZeroU64> in a pod type requires nightly
    pub client_order_ids: [u64; 128],
    pub referrer_rebates_accrued: u64,
}
unsafe impl Pod for OpenOrders {}
unsafe impl Zeroable for OpenOrders {}

impl OpenOrders {
    fn check_flags(&self) -> DexResult {
        let flags = BitFlags::from_bits(self.account_flags)
            .map_err(|_| DexErrorCode::InvalidMarketFlags)?;
        let required_flags = AccountFlag::Initialized | AccountFlag::OpenOrders;
        if flags != required_flags {
            Err(DexErrorCode::WrongOrdersAccount)?
        }
        Ok(())
    }

    fn init(&mut self, market: &[u64; 4], owner: &[u64; 4]) -> DexResult<()> {
        check_assert_eq!(&self.account_flags, &0)?;
        self.account_flags = (AccountFlag::Initialized | AccountFlag::OpenOrders).bits();
        self.market = *market;
        self.owner = *owner;
        self.native_coin_total = 0;
        self.native_coin_free = 0;
        self.native_pc_total = 0;
        self.native_pc_free = 0;
        self.free_slot_bits = std::u128::MAX;
        Ok(())
    }

    fn credit_locked_coin(&mut self, native_coin_amount: u64) {
        self.native_coin_total = self
            .native_coin_total
            .checked_add(native_coin_amount)
            .unwrap();
    }

    fn credit_locked_pc(&mut self, native_pc_amount: u64) {
        self.native_pc_total = self.native_pc_total.checked_add(native_pc_amount).unwrap();
    }

    fn lock_free_coin(&mut self, native_coin_amount: u64) {
        self.native_coin_free = self
            .native_coin_free
            .checked_sub(native_coin_amount)
            .unwrap();
    }

    fn lock_free_pc(&mut self, native_pc_amount: u64) {
        self.native_pc_free = self.native_pc_free.checked_sub(native_pc_amount).unwrap();
    }

    fn slot_is_free(&self, slot: u8) -> bool {
        let slot_mask = 1u128 << slot;
        self.free_slot_bits & slot_mask != 0
    }

    pub fn slot_side(&self, slot: u8) -> Option<Side> {
        let slot_mask = 1u128 << slot;
        if self.free_slot_bits & slot_mask != 0 {
            None
        } else if self.is_bid_bits & slot_mask != 0 {
            Some(Side::Bid)
        } else {
            Some(Side::Ask)
        }
    }

    fn remove_order(&mut self, slot: u8) -> DexResult {
        check_assert!(slot < 128)?;
        check_assert!(!self.slot_is_free(slot))?;

        let slot_mask = 1u128 << slot;
        self.orders[slot as usize] = 0;
        self.client_order_ids[slot as usize] = 0;
        self.free_slot_bits |= slot_mask;
        self.is_bid_bits &= !slot_mask;

        Ok(())
    }

    fn add_order(&mut self, id: u128, side: Side) -> DexResult<u8> {
        if self.free_slot_bits == 0 {
            Err(DexErrorCode::TooManyOpenOrders)?;
        }
        let slot = self.free_slot_bits.trailing_zeros();
        check_assert!(self.slot_is_free(slot as u8))?;
        let slot_mask = 1u128 << slot;
        self.free_slot_bits &= !slot_mask;
        match side {
            Side::Bid => {
                self.is_bid_bits |= slot_mask;
            }
            Side::Ask => {
                self.is_bid_bits &= !slot_mask;
            }
        };
        self.orders[slot as usize] = id;
        Ok(slot as u8)
    }
}

pub trait QueueHeader: Pod {
    type Item: Pod + Copy;

    fn head(&self) -> u64;
    fn head_mut(&mut self) -> &mut u64;
    fn count(&self) -> u64;
    fn count_mut(&mut self) -> &mut u64;

    fn incr_event_id(&mut self);
    fn decr_event_id(&mut self, n: u64);
}

pub struct Queue<'a, H: QueueHeader> {
    header: RefMut<'a, H>,
    buf: RefMut<'a, [H::Item]>,
}

impl<'a, H: QueueHeader> Queue<'a, H> {
    pub fn new(header: RefMut<'a, H>, buf: RefMut<'a, [H::Item]>) -> Self {
        Self { header, buf }
    }

    #[inline]
    pub fn len(&self) -> u64 {
        self.header.count()
    }

    #[inline]
    pub fn full(&self) -> bool {
        self.header.count() as usize == self.buf.len()
    }

    #[inline]
    pub fn empty(&self) -> bool {
        self.header.count() == 0
    }

    #[inline]
    pub fn push_back(&mut self, value: H::Item) -> Result<(), H::Item> {
        if self.full() {
            return Err(value);
        }
        let slot = ((self.header.head() + self.header.count()) as usize) % self.buf.len();
        self.buf[slot] = value;
        *self.header.count_mut() += 1;
        self.header.incr_event_id();
        Ok(())
    }

    #[inline]
    pub fn peek_front(&self) -> Option<&H::Item> {
        if self.empty() {
            return None;
        }
        Some(&self.buf[self.header.head() as usize])
    }

    #[inline]
    pub fn peek_front_mut(&mut self) -> Option<&mut H::Item> {
        if self.empty() {
            return None;
        }
        Some(&mut self.buf[self.header.head() as usize])
    }

    #[inline]
    pub fn pop_front(&mut self) -> Result<H::Item, ()> {
        if self.empty() {
            return Err(());
        }
        let value = self.buf[self.header.head() as usize];
        *self.header.count_mut() -= 1;
        let head = self.header.head_mut();
        *head = (*head + 1) % self.buf.len() as u64;
        Ok(value)
    }

    #[inline]
    pub fn revert_pushes(&mut self, desired_len: u64) -> DexResult<()> {
        check_assert!(desired_len <= self.header.count())?;
        let len_diff = self.header.count() - desired_len;
        *self.header.count_mut() = desired_len;
        self.header.decr_event_id(len_diff);
        Ok(())
    }

    pub fn iter(&self) -> impl Iterator<Item = &H::Item> {
        QueueIterator {
            queue: self,
            index: 0,
        }
    }
}

struct QueueIterator<'a, 'b, H: QueueHeader> {
    queue: &'b Queue<'a, H>,
    index: u64,
}

impl<'a, 'b, H: QueueHeader> Iterator for QueueIterator<'a, 'b, H> {
    type Item = &'b H::Item;
    fn next(&mut self) -> Option<Self::Item> {
        if self.index == self.queue.len() {
            None
        } else {
            let item = &self.queue.buf
                [(self.queue.header.head() + self.index) as usize % self.queue.buf.len()];
            self.index += 1;
            Some(item)
        }
    }
}

#[derive(Copy, Clone, Debug)]
#[repr(C)]
pub struct RequestQueueHeader {
    account_flags: u64, // Initialized, RequestQueue
    head: u64,
    count: u64,
    next_seq_num: u64,
}
unsafe impl Zeroable for RequestQueueHeader {}
unsafe impl Pod for RequestQueueHeader {}

impl QueueHeader for RequestQueueHeader {
    type Item = Request;

    fn head(&self) -> u64 {
        self.head
    }
    fn head_mut(&mut self) -> &mut u64 {
        &mut self.head
    }
    fn count(&self) -> u64 {
        self.count
    }
    fn count_mut(&mut self) -> &mut u64 {
        &mut self.count
    }
    #[inline(always)]
    fn incr_event_id(&mut self) {}
    #[inline(always)]
    fn decr_event_id(&mut self, _n: u64) {}
}

pub type RequestQueue<'a> = Queue<'a, RequestQueueHeader>;

impl RequestQueue<'_> {
    fn gen_order_id(&mut self, limit_price: u64, side: Side) -> u128 {
        let seq_num = self.gen_seq_num();
        let upper = (limit_price as u128) << 64;
        let lower = match side {
            Side::Bid => !seq_num,
            Side::Ask => seq_num,
        };
        upper | (lower as u128)
    }

    fn gen_seq_num(&mut self) -> u64 {
        let seq_num = self.header.next_seq_num;
        self.header.next_seq_num += 1;
        seq_num
    }
}

#[derive(Copy, Clone, BitFlags, Debug)]
#[repr(u8)]
enum RequestFlag {
    NewOrder = 0x01,
    CancelOrder = 0x02,
    Bid = 0x04,
    PostOnly = 0x08,
    ImmediateOrCancel = 0x10,
    DecrementTakeOnSelfTrade = 0x20,
}

#[derive(Copy, Clone, Debug)]
#[repr(C)]
pub struct Request {
    request_flags: u8,
    owner_slot: u8,
    fee_tier: u8,
    self_trade_behavior: u8,
    padding: [u8; 4],
    max_coin_qty_or_cancel_id: u64,
    native_pc_qty_locked: u64,
    order_id: u128,
    owner: [u64; 4],
    client_order_id: u64,
}
unsafe impl Zeroable for Request {}
unsafe impl Pod for Request {}

#[derive(Debug)]
pub enum RequestView<'a> {
    NewOrder {
        side: Side,
        order_type: OrderType,
        owner_slot: u8,
        fee_tier: FeeTier,
        order_id: &'a u128,
        max_coin_qty: NonZeroU64,
        native_pc_qty_locked: Option<NonZeroU64>,
        owner: &'a [u64; 4],
        client_order_id: Option<NonZeroU64>,
        self_trade_behavior: SelfTradeBehavior,
    },
    CancelOrder {
        side: Side,
        order_id: &'a u128,
        cancel_id: u64,
        expected_owner_slot: u8,
        expected_owner: &'a [u64; 4],
        client_order_id: Option<NonZeroU64>,
    },
}

impl Request {
    #[inline(always)]
    pub fn new(view: RequestView) -> Self {
        match view {
            RequestView::NewOrder {
                side,
                order_type,
                owner_slot,
                fee_tier,
                order_id,
                owner,
                max_coin_qty,
                native_pc_qty_locked,
                client_order_id,
                self_trade_behavior,
            } => {
                let mut flags = BitFlags::from_flag(RequestFlag::NewOrder);
                if side == Side::Bid {
                    flags.insert(RequestFlag::Bid);
                }
                match order_type {
                    OrderType::PostOnly => flags |= RequestFlag::PostOnly,
                    OrderType::ImmediateOrCancel => flags |= RequestFlag::ImmediateOrCancel,
                    OrderType::Limit => (),
                };

                Request {
                    request_flags: flags.bits(),
                    owner_slot,
                    fee_tier: fee_tier.into(),
                    self_trade_behavior: self_trade_behavior.into(),
                    padding: Zeroable::zeroed(),
                    order_id: *order_id,
                    owner: *owner,
                    max_coin_qty_or_cancel_id: max_coin_qty.get(),
                    native_pc_qty_locked: native_pc_qty_locked.map_or(0, NonZeroU64::get),
                    client_order_id: client_order_id.map_or(0, NonZeroU64::get),
                }
            }
            RequestView::CancelOrder {
                side,
                expected_owner_slot,
                order_id,
                expected_owner,
                cancel_id,
                client_order_id,
            } => {
                let mut flags = BitFlags::from_flag(RequestFlag::CancelOrder);
                if side == Side::Bid {
                    flags.insert(RequestFlag::Bid);
                }
                Request {
                    request_flags: flags.bits(),
                    max_coin_qty_or_cancel_id: cancel_id,
                    order_id: *order_id,
                    owner_slot: expected_owner_slot,
                    fee_tier: 0,
                    self_trade_behavior: 0,
                    owner: *expected_owner,
                    native_pc_qty_locked: 0,
                    padding: Zeroable::zeroed(),
                    client_order_id: client_order_id.map_or(0, NonZeroU64::get),
                }
            }
        }
    }

    #[inline(always)]
    pub fn as_view(&self) -> DexResult<RequestView> {
        let flags = BitFlags::from_bits(self.request_flags).unwrap();
        let side = if flags.contains(RequestFlag::Bid) {
            Side::Bid
        } else {
            Side::Ask
        };
        if flags.contains(RequestFlag::NewOrder) {
            let allowed_flags = {
                use RequestFlag::*;
                NewOrder | Bid | PostOnly | ImmediateOrCancel
            };
            check_assert!(allowed_flags.contains(flags))?;
            let post_only = flags.contains(RequestFlag::PostOnly);
            let ioc = flags.contains(RequestFlag::ImmediateOrCancel);
            let order_type = match (post_only, ioc) {
                (true, false) => OrderType::PostOnly,
                (false, true) => OrderType::ImmediateOrCancel,
                (false, false) => OrderType::Limit,
                (true, true) => unreachable!(),
            };
            let fee_tier = FeeTier::try_from_primitive(self.fee_tier).or(check_unreachable!())?;
            let self_trade_behavior =
                SelfTradeBehavior::try_from_primitive(self.self_trade_behavior)
                    .or(check_unreachable!())?;
            Ok(RequestView::NewOrder {
                side,
                order_type,
                owner_slot: self.owner_slot,
                fee_tier,
                self_trade_behavior,
                order_id: &self.order_id,
                owner: &self.owner,
                max_coin_qty: NonZeroU64::new(self.max_coin_qty_or_cancel_id).unwrap(),
                native_pc_qty_locked: NonZeroU64::new(self.native_pc_qty_locked),
                client_order_id: NonZeroU64::new(self.client_order_id),
            })
        } else {
            check_assert!(flags.contains(RequestFlag::CancelOrder))?;
            let allowed_flags = {
                use RequestFlag::*;
                CancelOrder | Bid
            };
            check_assert!(allowed_flags.contains(flags))?;
            Ok(RequestView::CancelOrder {
                side,
                cancel_id: self.max_coin_qty_or_cancel_id,
                order_id: &self.order_id,
                expected_owner_slot: self.owner_slot,
                expected_owner: &self.owner,
                client_order_id: NonZeroU64::new(self.client_order_id),
            })
        }
    }
}

#[derive(Copy, Clone, Debug)]
#[repr(C)]
pub struct EventQueueHeader {
    account_flags: u64, // Initialized, EventQueue
    head: u64,
    count: u64,
    seq_num: u64,
}
unsafe impl Zeroable for EventQueueHeader {}
unsafe impl Pod for EventQueueHeader {}

unsafe impl TriviallyTransmutable for EventQueueHeader {}
unsafe impl TriviallyTransmutable for RequestQueueHeader {}

impl QueueHeader for EventQueueHeader {
    type Item = Event;

    fn head(&self) -> u64 {
        self.head
    }
    fn head_mut(&mut self) -> &mut u64 {
        &mut self.head
    }
    fn count(&self) -> u64 {
        self.count
    }
    fn count_mut(&mut self) -> &mut u64 {
        &mut self.count
    }
    fn incr_event_id(&mut self) {
        self.seq_num += 1;
    }
    fn decr_event_id(&mut self, n: u64) {
        self.seq_num -= n;
    }
}

pub type EventQueue<'a> = Queue<'a, EventQueueHeader>;

#[derive(Copy, Clone, BitFlags, Debug)]
#[repr(u8)]
enum EventFlag {
    Fill = 0x1,
    Out = 0x2,
    Bid = 0x4,
    Maker = 0x8,
}

impl EventFlag {
    #[inline]
    fn from_side(side: Side) -> BitFlags<Self> {
        match side {
            Side::Bid => EventFlag::Bid.into(),
            Side::Ask => BitFlags::empty(),
        }
    }

    #[inline]
    fn flags_to_side(flags: BitFlags<Self>) -> Side {
        if flags.contains(EventFlag::Bid) {
            Side::Bid
        } else {
            Side::Ask
        }
    }
}

#[derive(Copy, Clone, Debug)]
#[repr(C)]
pub struct Event {
    event_flags: u8,
    owner_slot: u8,

    fee_tier: u8,

    _padding: [u8; 5],

    native_qty_released: u64,
    native_qty_paid: u64,
    native_fee_or_rebate: u64,

    order_id: u128,
    pub owner: [u64; 4],
    client_order_id: u64,
}
unsafe impl Zeroable for Event {}
unsafe impl Pod for Event {}

unsafe impl TriviallyTransmutable for Event {}
unsafe impl TriviallyTransmutable for Request {}

impl Event {
    #[inline(always)]
    pub fn new(view: EventView) -> Self {
        match view {
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
            } => {
                let maker_flag = if maker {
                    BitFlags::from_flag(EventFlag::Maker).bits()
                } else {
                    0
                };
                let event_flags =
                    (EventFlag::from_side(side) | EventFlag::Fill).bits() | maker_flag;
                Event {
                    event_flags,
                    owner_slot,
                    fee_tier: fee_tier.into(),

                    _padding: Zeroable::zeroed(),

                    native_qty_released: native_qty_received,
                    native_qty_paid,
                    native_fee_or_rebate,

                    order_id: *order_id,
                    owner: *owner,

                    client_order_id: client_order_id.map_or(0, NonZeroU64::get),
                }
            }

            EventView::Out {
                side,
                native_qty_unlocked,
                native_qty_still_locked,
                order_id,
                owner,
                owner_slot,
                client_order_id,
            } => {
                let event_flags = (EventFlag::from_side(side) | EventFlag::Out).bits();
                Event {
                    event_flags,
                    owner_slot,
                    fee_tier: 0,

                    _padding: Zeroable::zeroed(),

                    native_qty_released: native_qty_unlocked,
                    native_qty_paid: native_qty_still_locked,
                    native_fee_or_rebate: 0,

                    order_id: *order_id,
                    owner: *owner,
                    client_order_id: client_order_id.map_or(0, NonZeroU64::get),
                }
            }
        }
    }

    #[inline(always)]
    pub fn as_view(&self) -> DexResult<EventView> {
        let flags = BitFlags::from_bits(self.event_flags).unwrap();
        let side = EventFlag::flags_to_side(flags);
        let client_order_id = NonZeroU64::new(self.client_order_id);
        if flags.contains(EventFlag::Fill) {
            let allowed_flags = {
                use EventFlag::*;
                Fill | Bid | Maker
            };
            check_assert!(allowed_flags.contains(flags))?;

            return Ok(EventView::Fill {
                side,
                maker: flags.contains(EventFlag::Maker),
                native_qty_paid: self.native_qty_paid,
                native_qty_received: self.native_qty_released,
                native_fee_or_rebate: self.native_fee_or_rebate,

                order_id: &self.order_id,
                owner: &self.owner,

                owner_slot: self.owner_slot,
                fee_tier: self.fee_tier.try_into().or(check_unreachable!())?,
                client_order_id,
            });
        }
        let allowed_flags = {
            use EventFlag::*;
            Out | Bid | Maker
        };
        check_assert!(allowed_flags.contains(flags))?;
        Ok(EventView::Out {
            side,
            native_qty_unlocked: self.native_qty_released,
            native_qty_still_locked: self.native_qty_paid,

            order_id: &self.order_id,
            owner: &self.owner,

            owner_slot: self.owner_slot,
            client_order_id,
        })
    }
}

#[derive(Debug)]
pub enum EventView<'a> {
    Fill {
        side: Side,
        maker: bool,
        native_qty_paid: u64,
        native_qty_received: u64,
        native_fee_or_rebate: u64,
        order_id: &'a u128,
        owner: &'a [u64; 4],
        owner_slot: u8,
        fee_tier: FeeTier,
        client_order_id: Option<NonZeroU64>,
    },
    Out {
        side: Side,
        native_qty_unlocked: u64,
        native_qty_still_locked: u64,
        order_id: &'a u128,
        owner: &'a [u64; 4],
        owner_slot: u8,
        client_order_id: Option<NonZeroU64>,
    },
}

impl<'a> EventView<'a> {
    fn side(&self) -> Side {
        match self {
            &EventView::Fill { side, .. } | &EventView::Out { side, .. } => side,
        }
    }
}

#[derive(Copy, Clone)]
#[repr(C)]
struct OrderBookStateHeader {
    account_flags: u64, // Initialized, (Bids or Asks)
}
unsafe impl Zeroable for OrderBookStateHeader {}
unsafe impl Pod for OrderBookStateHeader {}

pub enum State {}

fn gen_vault_signer_seeds<'a>(nonce: &'a u64, market: &'a Pubkey) -> [&'a [u8]; 2] {
    [market.as_ref(), bytes_of(nonce)]
}

#[cfg(not(any(test, feature = "fuzz")))]
#[inline]
pub fn gen_vault_signer_key(
    nonce: u64,
    market: &Pubkey,
    program_id: &Pubkey,
) -> Result<Pubkey, ProgramError> {
    let seeds = gen_vault_signer_seeds(&nonce, market);
    Ok(Pubkey::create_program_address(&seeds, program_id)?)
}

#[cfg(any(test, feature = "fuzz"))]
pub fn gen_vault_signer_key(
    nonce: u64,
    market: &Pubkey,
    _program_id: &Pubkey,
) -> Result<Pubkey, ProgramError> {
    gen_vault_signer_seeds(&nonce, market);
    Ok(Pubkey::default())
}

#[cfg(not(feature = "fuzz"))]
#[cfg(feature = "program")]
fn invoke_spl_token(
    instruction: &solana_sdk::instruction::Instruction,
    account_infos: &[AccountInfo],
    signers_seeds: &[&[&[u8]]],
) -> solana_sdk::entrypoint::ProgramResult {
    solana_sdk::program::invoke_signed(instruction, account_infos, signers_seeds)
}

#[cfg(feature = "fuzz")]
fn invoke_spl_token(
    instruction: &solana_sdk::instruction::Instruction,
    account_infos: &[AccountInfo],
    _signers_seeds: &[&[&[u8]]],
) -> solana_sdk::entrypoint::ProgramResult {
    assert_eq!(instruction.program_id, spl_token::ID);
    let account_infos: Vec<AccountInfo> = instruction
        .accounts
        .iter()
        .map(|meta| {
            account_infos
                .iter()
                .find(|info| *info.key == meta.pubkey)
                .unwrap()
                .clone()
        })
        .collect();
    info!("invoking...");
    spl_token::processor::Processor::process(
        &instruction.program_id,
        &account_infos,
        &instruction.data,
    )?;
    info!("invoked");
    Ok(())
}

#[cfg(not(feature = "client"))]
fn send_from_vault<'a, 'b: 'a>(
    native_amount: u64,
    recipient: account_parser::TokenAccount<'a, 'b>,
    vault: account_parser::TokenAccount<'a, 'b>,
    spl_token_program: account_parser::SplTokenProgram<'a, 'b>,
    vault_signer: account_parser::VaultSigner<'a, 'b>,
    vault_signer_seeds: &[&[u8]],
) -> DexResult {
    let deposit_instruction = spl_token::instruction::transfer(
        &spl_token::ID,
        vault.inner().key,
        recipient.inner().key,
        &vault_signer.inner().key,
        &[],
        native_amount,
    )?;
    let accounts: &[AccountInfo] = &[
        vault.inner().clone(),
        recipient.inner().clone(),
        vault_signer.inner().clone(),
        spl_token_program.inner().clone(),
    ];
    invoke_spl_token(&deposit_instruction, &accounts[..], &[vault_signer_seeds])
        .map_err(|_| DexErrorCode::TransferFailed)?;
    Ok(())
}

pub mod account_parser {
    use super::*;

    macro_rules! declare_validated_account_wrapper {
        ($WrapperT:ident, $validate:expr $(, $a:ident : $t:ty)*) => {
            #[derive(Copy, Clone)]
            pub struct $WrapperT<'a, 'b: 'a>(&'a AccountInfo<'b>);
            impl<'a, 'b: 'a> $WrapperT<'a, 'b> {
                fn new(account: &'a AccountInfo<'b> $(,$a: $t)*) -> DexResult<Self> {
                    let validate_result: DexResult = $validate(account $(,$a)*);
                    validate_result?;
                    Ok($WrapperT(account))
                }

                #[inline(always)]
                pub fn inner(self) -> &'a AccountInfo<'b> {
                    self.0
                }
            }
        }
    }

    declare_validated_account_wrapper!(SplTokenProgram, |account: &AccountInfo| {
        check_assert_eq!(*account.key, spl_token::ID)?;
        Ok(())
    });

    declare_validated_account_wrapper!(TokenMint, |mint: &AccountInfo| {
        check_assert_eq!(*mint.owner, spl_token::ID)?;
        let data = mint.try_borrow_data()?;
        check_assert_eq!(data.len(), spl_token::state::Mint::LEN)?;

        let is_initialized = data[0x2d];
        check_assert_eq!(is_initialized, 1u8)?;
        Ok(())
    });

    declare_validated_account_wrapper!(TokenAccount, |account: &AccountInfo| {
        check_assert_eq!(*account.owner, spl_token::ID)?;
        let data = account.try_borrow_data()?;
        check_assert_eq!(data.len(), spl_token::state::Account::LEN)?;

        let is_initialized = data[0x6c];
        check_assert_eq!(is_initialized, 1u8)?;
        Ok(())
    });

    macro_rules! declare_validated_token_account_wrapper {
        ($WrapperT:ident, $validate:expr $(, $a:ident : $t:ty)*) => {
            #[derive(Copy, Clone)]
            pub struct $WrapperT<'a, 'b: 'a>(TokenAccount<'a, 'b>);
            impl<'a, 'b: 'a> $WrapperT<'a, 'b> {
                fn new(token_account: TokenAccount<'a, 'b> $(,$a: $t)*) -> DexResult<Self> {
                    let validate_result: DexResult = $validate(token_account $(,$a)*);
                    validate_result?;
                    Ok($WrapperT(token_account))
                }

                fn from_account(account: &'a AccountInfo<'b> $(,$a: $t)*) -> DexResult<Self> {
                    let token_account = TokenAccount::new(account)?;
                    Self::new(token_account $(,$a)*)
                }

                #[inline(always)]
                pub fn token_account(self) -> TokenAccount<'a, 'b> {
                    self.0
                }

                #[inline(always)]
                pub fn account(self) -> &'a AccountInfo<'b> {
                    self.0.inner()
                }
            }
        }
    }

    declare_validated_account_wrapper!(RentSysvarAccount, |account: &AccountInfo| {
        check_assert!(Rent::check_id(account.key))?;
        Ok(())
    });

    declare_validated_account_wrapper!(SignerAccount, |account: &AccountInfo| {
        check_assert!(account.is_signer)?;
        Ok(())
    });

    declare_validated_account_wrapper!(SigningFeeSweeper, |account: &AccountInfo| {
        check_assert!(account.is_signer)?;
        check_assert_eq!(account.key, &fee_sweeper::ID)?;
        Ok(())
    });

    declare_validated_account_wrapper!(SigningDisableAuthority, |account: &AccountInfo| {
        check_assert!(account.is_signer)?;
        check_assert_eq!(account.key, &disable_authority::ID)?;
        Ok(())
    });

    declare_validated_token_account_wrapper!(
        CoinVault,
        |token_account: TokenAccount, market: &MarketState| {
            market.check_coin_vault(token_account)
        },
        market: &MarketState
    );

    declare_validated_token_account_wrapper!(
        PcVault,
        |token_account: TokenAccount, market: &MarketState| {
            market.check_pc_vault(token_account)
        },
        market: &MarketState
    );

    declare_validated_token_account_wrapper!(
        CoinWallet,
        |token_account: TokenAccount, market: &MarketState| {
            market.check_coin_payer(token_account)
        },
        market: &MarketState
    );

    declare_validated_token_account_wrapper!(
        PcWallet,
        |token_account: TokenAccount, market: &MarketState| {
            market.check_pc_payer(token_account)
        },
        market: &MarketState
    );

    declare_validated_account_wrapper!(
        VaultSigner,
        |account: &AccountInfo, market: &MarketState, program_id: &Pubkey| {
            let vault_signer_key =
                gen_vault_signer_key(market.vault_signer_nonce, &market.pubkey(), program_id)?;
            Ok(check_assert_eq!(&vault_signer_key, account.key)?)
        },
        market: &MarketState,
        program_id: &Pubkey
    );

    impl<'a, 'b: 'a> TokenAccount<'a, 'b> {
        pub fn balance(self) -> DexResult<u64> {
            let data = self.inner().try_borrow_data()?;
            Ok(u64::from_le_bytes(*array_ref![data, 64, 8]))
        }
    }

    #[derive(Copy, Clone)]
    pub struct TokenAccountAndMint<'a, 'b: 'a> {
        account: TokenAccount<'a, 'b>,
        mint: TokenMint<'a, 'b>,
    }

    impl<'a, 'b: 'a> TokenAccountAndMint<'a, 'b> {
        fn new(account: TokenAccount<'a, 'b>, mint: TokenMint<'a, 'b>) -> DexResult<Self> {
            let account_data = account.0.try_borrow_data()?;
            check_assert_eq!(mint.0.key.as_ref(), &account_data[..32])?;
            Ok(TokenAccountAndMint { account, mint })
        }

        pub fn get_account(self) -> TokenAccount<'a, 'b> {
            self.account
        }

        pub fn get_mint(self) -> TokenMint<'a, 'b> {
            self.mint
        }
    }

    pub struct InitializeMarketArgs<'a, 'b: 'a> {
        pub program_id: &'a Pubkey,
        pub instruction: &'a InitializeMarketInstruction,
        serum_dex_accounts: &'a [AccountInfo<'b>; 5],
        pub coin_vault_and_mint: TokenAccountAndMint<'a, 'b>,
        pub pc_vault_and_mint: TokenAccountAndMint<'a, 'b>,
    }

    impl<'a, 'b: 'a> InitializeMarketArgs<'a, 'b> {
        pub fn new(
            program_id: &'a Pubkey,
            instruction: &'a InitializeMarketInstruction,
            accounts: &'a [AccountInfo<'b>],
        ) -> DexResult<Self> {
            check_assert_eq!(accounts.len(), 9)?;
            let accounts = array_ref![accounts, 0, 9];
            let (unchecked_serum_dex_accounts, unchecked_vaults, unchecked_mints) =
                array_refs![accounts, 5, 2, 2];
            let mut checked_vaults = [None, None];
            for account in unchecked_serum_dex_accounts {
                check_assert_eq!(account.owner, program_id)?;
                let data = account.try_borrow_data()?;
                check_assert_eq!(data.len() % 8, 4)?;
                check_assert!(data.len() >= 20)?;
                let (padding5, header, _, padding7) = array_refs![&data, 5, 8; .. ; 7];
                check_assert_eq!(*padding5, [0u8; 5])?;
                check_assert_eq!(*header, [0u8; 8])?;
                check_assert_eq!(*padding7, [0u8; 7])?;
            }
            let serum_dex_accounts = unchecked_serum_dex_accounts;
            let vault_owner_key_bytes = gen_vault_signer_key(
                instruction.vault_signer_nonce,
                serum_dex_accounts[0].key,
                program_id,
            )?
            .to_bytes();
            for i in 0..=1 {
                let vault = TokenAccount::new(&unchecked_vaults[i])?;
                let mint = TokenMint::new(&unchecked_mints[i])?;

                // check that the vaults are owned by the market's withdrawal authority key
                let vault_data = vault.0.try_borrow_data()?;
                let vault_owner = array_ref![vault_data, 0x20, 0x20];
                check_assert_eq!(vault_owner, &vault_owner_key_bytes)?;

                // check that the vault has no delegate
                let delegate_tag = array_ref![vault_data, 0x48, 0x4];
                check_assert_eq!(*delegate_tag, [0u8; 4])?;

                checked_vaults[i] = Some(TokenAccountAndMint::new(vault, mint)?);
            }
            let [coin_vault_and_mint, pc_vault_and_mint] = match checked_vaults {
                [Some(cvm), Some(pvm)] => [cvm, pvm],
                _ => check_unreachable!()?,
            };

            Ok(InitializeMarketArgs {
                program_id,
                instruction,
                serum_dex_accounts,
                coin_vault_and_mint,
                pc_vault_and_mint,
            })
        }

        pub fn get_dex_accounts(&self) -> &'a [AccountInfo<'b>] {
            self.serum_dex_accounts as &[AccountInfo]
        }

        pub fn get_market(&self) -> &'a AccountInfo<'b> {
            &self.serum_dex_accounts[0]
        }

        pub fn get_req_q(&self) -> &'a AccountInfo<'b> {
            &self.serum_dex_accounts[1]
        }

        pub fn get_event_q(&self) -> &'a AccountInfo<'b> {
            &self.serum_dex_accounts[2]
        }

        pub fn get_bids(&self) -> &'a AccountInfo<'b> {
            &self.serum_dex_accounts[3]
        }

        pub fn get_asks(&self) -> &'a AccountInfo<'b> {
            &self.serum_dex_accounts[4]
        }
    }

    pub struct NewOrderArgs<'a, 'b: 'a> {
        pub instruction: &'a NewOrderInstructionV2,
        pub market: &'a mut MarketState,
        pub open_orders: &'a mut OpenOrders,
        pub open_orders_address: &'a [u64; 4],
        pub owner: SignerAccount<'a, 'b>,
        pub req_q: RequestQueue<'a>,
        pub payer: TokenAccount<'a, 'b>,
        pub coin_vault: CoinVault<'a, 'b>,
        pub pc_vault: PcVault<'a, 'b>,
        pub spl_token_program: SplTokenProgram<'a, 'b>,
        pub fee_tier: FeeTier,
    }
    impl<'a, 'b: 'a> NewOrderArgs<'a, 'b> {
        pub fn with_parsed_args<T>(
            program_id: &'a Pubkey,
            instruction: &'a NewOrderInstructionV2,
            accounts: &'a [AccountInfo<'b>],
            f: impl FnOnce(NewOrderArgs) -> DexResult<T>,
        ) -> DexResult<T> {
            check_assert!(accounts.len() == 9 || accounts.len() == 10)?;
            let (fixed_accounts, fee_discount_account): (
                &'a [AccountInfo<'b>; 9],
                &'a [AccountInfo<'b>],
            ) = array_refs![accounts, 9; .. ;];
            let &[
                ref market_acc,
                ref open_orders_acc,
                ref req_q_acc,
                ref payer_acc,
                ref owner_acc,
                ref coin_vault_acc,
                ref pc_vault_acc,
                ref spl_token_program_acc,
                ref rent_sysvar_acc,
            ]: &'a [AccountInfo<'b>; 9] = fixed_accounts;
            let srm_or_msrm_account = match fee_discount_account {
                &[] => None,
                &[ref account] => Some(TokenAccount::new(account)?),
                _ => check_unreachable!()?,
            };

            let mut market: RefMut<'a, MarketState> = MarketState::load(market_acc, program_id)?;
            let rent = {
                let rent_sysvar = RentSysvarAccount::new(rent_sysvar_acc)?;
                Rent::from_account_info(rent_sysvar.inner()).or(check_unreachable!())?
            };
            let owner = SignerAccount::new(owner_acc)?;
            let fee_tier =
                market.load_fee_tier(&owner.inner().key.to_aligned_bytes(), srm_or_msrm_account)?;
            let mut open_orders = market.load_orders_mut(
                open_orders_acc,
                Some(owner.inner()),
                program_id,
                Some(rent),
            )?;
            let ref open_orders_address = open_orders_acc.key.to_aligned_bytes();
            let req_q = market.load_request_queue_mut(req_q_acc)?;

            let payer = TokenAccount::new(payer_acc)?;
            match instruction.side {
                Side::Bid => market.check_pc_payer(payer).or(check_unreachable!())?,
                Side::Ask => market.check_coin_payer(payer).or(check_unreachable!())?,
            };
            let coin_vault = CoinVault::from_account(coin_vault_acc, &market)?;
            let pc_vault = PcVault::from_account(pc_vault_acc, &market)?;
            market.check_enabled()?;
            let spl_token_program = SplTokenProgram::new(spl_token_program_acc)?;
            let args = NewOrderArgs {
                instruction,
                market: market.deref_mut(),
                open_orders: open_orders.deref_mut(),
                open_orders_address,
                owner,
                req_q,
                payer,
                coin_vault,
                pc_vault,
                spl_token_program,
                fee_tier,
            };
            f(args)
        }
    }

    pub struct MatchOrdersArgs<'a> {
        pub limit: u16,
        pub order_book_state: OrderBookState<'a>,
        pub req_q: RequestQueue<'a>,
        pub event_q: EventQueue<'a>,
    }
    impl<'a> MatchOrdersArgs<'a> {
        pub fn with_parsed_args<'b, T>(
            program_id: &'a Pubkey,
            accounts: &'a [AccountInfo<'b>],
            limit: u16,
            f: impl FnOnce(MatchOrdersArgs) -> DexResult<T>,
        ) -> DexResult<T> {
            check_assert!(accounts.len() >= 5)?;
            #[rustfmt::skip]
            let &[
                ref market_acc,
                ref req_q_acc,
                ref event_q_acc,
                ref bids_acc,
                ref asks_acc
            ] = array_ref![accounts, 0, 5];
            let mut market = MarketState::load(market_acc, program_id).or(check_unreachable!())?;
            let req_q = market
                .load_request_queue_mut(req_q_acc)
                .or(check_unreachable!())?;
            let event_q = market
                .load_event_queue_mut(event_q_acc)
                .or(check_unreachable!())?;
            let mut bids = market.load_bids_mut(bids_acc).or(check_unreachable!())?;
            let mut asks = market.load_asks_mut(asks_acc).or(check_unreachable!())?;

            let order_book_state = OrderBookState {
                bids: bids.deref_mut(),
                asks: asks.deref_mut(),
                market_state: market.deref_mut(),
            };

            let args = MatchOrdersArgs {
                limit,
                order_book_state,
                req_q,
                event_q,
            };
            f(args)
        }
    }

    pub struct ConsumeEventsArgs<'a, 'b: 'a> {
        pub limit: u16,
        pub program_id: &'a Pubkey,
        pub open_orders_accounts: &'a [AccountInfo<'b>],
        pub market: &'a mut MarketState,
        pub event_q: EventQueue<'a>,
    }
    impl<'a, 'b: 'a> ConsumeEventsArgs<'a, 'b> {
        pub fn with_parsed_args<T>(
            program_id: &'a Pubkey,
            accounts: &'a [AccountInfo<'b>],
            limit: u16,
            f: impl FnOnce(ConsumeEventsArgs) -> DexResult<T>,
        ) -> DexResult<T> {
            check_assert!(accounts.len() >= 5)?;
            #[rustfmt::skip]
            let (
                &[],
                open_orders_accounts,
                &[ref market_acc],
                &[ref event_q_acc],
                _unused
            ) = array_refs![accounts, 0; .. ; 1, 1, 2];
            let mut market = MarketState::load(market_acc, program_id)?;
            let event_q = market.load_event_queue_mut(event_q_acc)?;
            let args = ConsumeEventsArgs {
                limit,
                program_id,
                open_orders_accounts,
                market: market.deref_mut(),
                event_q,
            };
            f(args)
        }
    }

    pub struct CancelOrderArgs<'a, 'b: 'a> {
        pub instruction: &'a CancelOrderInstruction,
        pub open_orders: &'a mut OpenOrders,
        pub open_orders_address: &'a [u64; 4],
        pub req_q: RequestQueue<'a>,
        pub orders_owner: SignerAccount<'a, 'b>,
    }
    impl<'a, 'b: 'a> CancelOrderArgs<'a, 'b> {
        pub fn with_parsed_args<T>(
            program_id: &'a Pubkey,
            accounts: &'a [AccountInfo<'b>],
            instruction: &'a CancelOrderInstruction,
            f: impl FnOnce(CancelOrderArgs) -> DexResult<T>,
        ) -> DexResult<T> {
            check_assert_eq!(accounts.len(), 4)?;
            #[rustfmt::skip]
            let &[
                ref market_acc,
                ref open_orders_acc,
                ref req_q_acc,
                ref owner_acc
            ] = array_ref![accounts, 0, 4];
            let market: RefMut<'a, MarketState> = MarketState::load(market_acc, program_id)?;
            let owner = SignerAccount::new(owner_acc)?;
            let mut open_orders =
                market.load_orders_mut(open_orders_acc, Some(owner.inner()), program_id, None)?;
            let ref open_orders_address = open_orders_acc.key.to_aligned_bytes();
            let req_q = market.load_request_queue_mut(req_q_acc)?;
            let args = CancelOrderArgs {
                instruction,
                open_orders: open_orders.deref_mut(),
                open_orders_address,
                req_q,
                orders_owner: owner,
            };
            f(args)
        }
    }

    pub struct CancelOrderByClientIdArgs<'a, 'b: 'a> {
        pub client_order_id: NonZeroU64,
        pub open_orders: &'a mut OpenOrders,
        pub open_orders_address: &'a [u64; 4],
        pub req_q: RequestQueue<'a>,
        pub orders_owner: SignerAccount<'a, 'b>,
    }
    impl<'a, 'b: 'a> CancelOrderByClientIdArgs<'a, 'b> {
        pub fn with_parsed_args<T>(
            program_id: &'a Pubkey,
            accounts: &'a [AccountInfo<'b>],
            client_id: u64,
            f: impl FnOnce(CancelOrderByClientIdArgs) -> DexResult<T>,
        ) -> DexResult<T> {
            let client_order_id = NonZeroU64::new(client_id).ok_or(assertion_error!())?;
            check_assert_eq!(accounts.len(), 4)?;
            #[rustfmt::skip]
            let &[
                ref market_acc,
                ref open_orders_acc,
                ref req_q_acc,
                ref owner_acc
            ] = array_ref![accounts, 0, 4];
            let market: RefMut<'a, MarketState> = MarketState::load(market_acc, program_id)?;
            let owner = SignerAccount::new(owner_acc)?;
            let mut open_orders =
                market.load_orders_mut(open_orders_acc, Some(owner.inner()), program_id, None)?;
            let ref open_orders_address = open_orders_acc.key.to_aligned_bytes();
            let req_q = market.load_request_queue_mut(req_q_acc)?;
            let args = CancelOrderByClientIdArgs {
                client_order_id,
                open_orders: open_orders.deref_mut(),
                open_orders_address,
                req_q,
                orders_owner: owner,
            };
            f(args)
        }
    }

    pub struct SettleFundsArgs<'a, 'b: 'a> {
        pub market: &'a mut MarketState,
        pub open_orders: &'a mut OpenOrders,
        pub coin_vault: CoinVault<'a, 'b>,
        pub pc_vault: PcVault<'a, 'b>,
        pub coin_wallet: CoinWallet<'a, 'b>,
        pub pc_wallet: PcWallet<'a, 'b>,
        pub vault_signer: VaultSigner<'a, 'b>,
        pub spl_token_program: SplTokenProgram<'a, 'b>,
        pub referrer: Option<PcWallet<'a, 'b>>,
    }
    impl<'a, 'b: 'a> SettleFundsArgs<'a, 'b> {
        pub fn with_parsed_args<T>(
            program_id: &'a Pubkey,
            accounts: &'a [AccountInfo<'b>],
            f: impl FnOnce(SettleFundsArgs) -> DexResult<T>,
        ) -> DexResult<T> {
            check_assert!(accounts.len() == 9 || accounts.len() == 10)?;
            #[rustfmt::skip]
            let (&[
                ref market_acc,
                ref open_orders_acc,
                ref owner_acc,
                ref coin_vault_acc,
                ref pc_vault_acc,
                ref coin_wallet_acc,
                ref pc_wallet_acc,
                ref vault_signer_acc,
                ref spl_token_program_acc,
            ], remaining_accounts) = array_refs![accounts, 9; ..;];
            let spl_token_program = SplTokenProgram::new(spl_token_program_acc)?;
            let mut market = MarketState::load(market_acc, program_id)?;
            let owner = SignerAccount::new(owner_acc).or(check_unreachable!())?;

            let coin_vault =
                CoinVault::from_account(coin_vault_acc, &market).or(check_unreachable!())?;
            let pc_vault = PcVault::from_account(pc_vault_acc, &market).or(check_unreachable!())?;
            let coin_wallet =
                CoinWallet::from_account(coin_wallet_acc, &market).or(check_unreachable!())?;
            let pc_wallet =
                PcWallet::from_account(pc_wallet_acc, &market).or(check_unreachable!())?;

            let referrer = match remaining_accounts {
                &[] => None,
                &[ref referrer_acc] => {
                    Some(PcWallet::from_account(referrer_acc, &market).or(check_unreachable!())?)
                }
                _ => check_unreachable!()?,
            };

            let vault_signer = VaultSigner::new(vault_signer_acc, &market, program_id)?;

            let mut open_orders =
                market.load_orders_mut(open_orders_acc, Some(owner.inner()), program_id, None)?;

            let args = SettleFundsArgs {
                market: market.deref_mut(),
                open_orders: open_orders.deref_mut(),
                coin_vault,
                pc_vault,
                coin_wallet,
                pc_wallet,
                vault_signer,
                spl_token_program,
                referrer,
            };
            f(args)
        }
    }

    pub struct DisableMarketArgs<'a, 'b: 'a> {
        pub market: &'a mut MarketState,
        pub authorization: SigningDisableAuthority<'a, 'b>,
    }
    impl<'a, 'b: 'a> DisableMarketArgs<'a, 'b> {
        pub fn with_parsed_args<T>(
            program_id: &'a Pubkey,
            accounts: &'a [AccountInfo<'b>],
            f: impl FnOnce(DisableMarketArgs) -> DexResult<T>,
        ) -> DexResult<T> {
            check_assert_eq!(accounts.len(), 2)?;
            let &[ref market_acc, ref signer_acc] = array_ref![accounts, 0, 2];
            let mut market = MarketState::load(market_acc, program_id)?;
            let authorization = SigningDisableAuthority::new(signer_acc)?;

            let args = DisableMarketArgs {
                market: market.deref_mut(),
                authorization,
            };
            f(args)
        }
    }

    pub struct SweepFeesArgs<'a, 'b: 'a> {
        pub market: &'a mut MarketState,
        pub pc_vault: PcVault<'a, 'b>,
        pub fee_receiver: PcWallet<'a, 'b>,
        pub vault_signer: VaultSigner<'a, 'b>,
        pub spl_token_program: SplTokenProgram<'a, 'b>,
        pub authorization: SigningFeeSweeper<'a, 'b>,
    }
    impl<'a, 'b: 'a> SweepFeesArgs<'a, 'b> {
        pub fn with_parsed_args<T>(
            program_id: &'a Pubkey,
            accounts: &'a [AccountInfo<'b>],
            f: impl FnOnce(SweepFeesArgs) -> DexResult<T>,
        ) -> DexResult<T> {
            check_assert_eq!(accounts.len(), 6)?;
            #[rustfmt::skip]
            let &[
                ref market_acc,
                ref pc_vault_acc,
                ref sweep_authority_acc,
                ref pc_wallet_acc,
                ref vault_signer_acc,
                ref spl_token_program
            ] = array_ref![accounts, 0, 6];

            let mut market = MarketState::load(market_acc, program_id)?;
            let pc_vault = PcVault::from_account(pc_vault_acc, &market)?;
            let fee_receiver = PcWallet::from_account(pc_wallet_acc, &market)?;
            let vault_signer = VaultSigner::new(vault_signer_acc, &market, program_id)?;
            let spl_token_program = SplTokenProgram::new(spl_token_program)?;
            let authorization = SigningFeeSweeper::new(sweep_authority_acc)?;

            let args = SweepFeesArgs {
                market: market.deref_mut(),
                pc_vault,
                fee_receiver,
                vault_signer,
                spl_token_program,
                authorization,
            };
            f(args)
        }
    }
}

#[inline]
fn remove_slop<T: Pod>(bytes: &[u8]) -> &[T] {
    let slop = bytes.len() % size_of::<T>();
    let new_len = bytes.len() - slop;
    cast_slice(&bytes[..new_len])
}

#[inline]
fn remove_slop_mut<T: Pod>(bytes: &mut [u8]) -> &mut [T] {
    let slop = bytes.len() % size_of::<T>();
    let new_len = bytes.len() - slop;
    cast_slice_mut(&mut bytes[..new_len])
}

#[cfg_attr(not(feature = "program"), allow(unused))]
impl State {
    #[cfg(feature = "program")]
    pub fn process(program_id: &Pubkey, accounts: &[AccountInfo], input: &[u8]) -> DexResult {
        let instruction = MarketInstruction::unpack(input).ok_or(ProgramError::InvalidArgument)?;
        match instruction {
            MarketInstruction::InitializeMarket(ref inner) => Self::process_initialize_market(
                account_parser::InitializeMarketArgs::new(program_id, inner, accounts)?,
            )?,
            MarketInstruction::NewOrder(inner) => {
                let new_order_v2 = inner.add_self_trade_behavior(SelfTradeBehavior::DecrementTake);
                account_parser::NewOrderArgs::with_parsed_args(
                    program_id,
                    &new_order_v2,
                    accounts,
                    Self::process_new_order,
                )?
            }
            MarketInstruction::NewOrderV2(ref inner) => {
                account_parser::NewOrderArgs::with_parsed_args(
                    program_id,
                    inner,
                    accounts,
                    Self::process_new_order,
                )?
            }
            MarketInstruction::MatchOrders(limit) => {
                account_parser::MatchOrdersArgs::with_parsed_args(
                    program_id,
                    accounts,
                    limit,
                    Self::process_match_orders,
                )?
            }
            MarketInstruction::ConsumeEvents(limit) => {
                account_parser::ConsumeEventsArgs::with_parsed_args(
                    program_id,
                    accounts,
                    limit,
                    Self::process_consume_events,
                )?
            }
            MarketInstruction::CancelOrder(ref inner) => {
                account_parser::CancelOrderArgs::with_parsed_args(
                    program_id,
                    accounts,
                    inner,
                    Self::process_cancel_order,
                )?
            }
            MarketInstruction::SettleFunds => account_parser::SettleFundsArgs::with_parsed_args(
                program_id,
                accounts,
                Self::process_settle_funds,
            )?,
            MarketInstruction::CancelOrderByClientId(client_id) => {
                account_parser::CancelOrderByClientIdArgs::with_parsed_args(
                    program_id,
                    accounts,
                    client_id,
                    Self::process_cancel_order_by_client_id,
                )?
            }
            MarketInstruction::DisableMarket => {
                account_parser::DisableMarketArgs::with_parsed_args(
                    program_id,
                    accounts,
                    Self::process_disable_market,
                )?
            }
            MarketInstruction::SweepFees => account_parser::SweepFeesArgs::with_parsed_args(
                program_id,
                accounts,
                Self::process_sweep_fees,
            )?,
        };
        Ok(())
    }

    #[cfg(feature = "program")]
    fn process_settle_funds(args: account_parser::SettleFundsArgs) -> DexResult {
        let account_parser::SettleFundsArgs {
            market,
            mut open_orders,
            coin_vault,
            pc_vault,
            coin_wallet,
            pc_wallet,
            vault_signer,
            spl_token_program,
            referrer,
        } = args;

        let native_coin_amount = open_orders.native_coin_free;
        let native_pc_amount = open_orders.native_pc_free;

        market.coin_deposits_total -= native_coin_amount;
        market.pc_deposits_total -= native_pc_amount;

        open_orders.native_coin_free = 0;
        open_orders.native_pc_free = 0;

        open_orders.native_coin_total = open_orders
            .native_coin_total
            .checked_sub(native_coin_amount)
            .unwrap();
        open_orders.native_pc_total = open_orders
            .native_pc_total
            .checked_sub(native_pc_amount)
            .unwrap();

        let token_infos: [(
            u64,
            account_parser::TokenAccount,
            account_parser::TokenAccount,
        ); 2] = [
            (
                native_coin_amount,
                coin_wallet.token_account(),
                coin_vault.token_account(),
            ),
            (
                native_pc_amount,
                pc_wallet.token_account(),
                pc_vault.token_account(),
            ),
        ];

        let market_pubkey = market.pubkey();
        let vault_signer_seeds = gen_vault_signer_seeds(&market.vault_signer_nonce, &market_pubkey);

        for &(token_amount, wallet_account, vault) in token_infos.iter() {
            send_from_vault(
                token_amount,
                wallet_account,
                vault,
                spl_token_program,
                vault_signer,
                &vault_signer_seeds,
            )?;
        }

        match referrer {
            Some(referrer_pc_wallet) if open_orders.referrer_rebates_accrued > 0 => {
                send_from_vault(
                    open_orders.referrer_rebates_accrued,
                    referrer_pc_wallet.token_account(),
                    pc_vault.token_account(),
                    spl_token_program,
                    vault_signer,
                    &vault_signer_seeds,
                )?;
            }
            _ => {
                market.pc_fees_accrued += open_orders.referrer_rebates_accrued;
            }
        };
        market.referrer_rebates_accrued -= open_orders.referrer_rebates_accrued;
        open_orders.referrer_rebates_accrued = 0;

        Ok(())
    }

    fn process_cancel_order_by_client_id(
        args: account_parser::CancelOrderByClientIdArgs,
    ) -> DexResult {
        let account_parser::CancelOrderByClientIdArgs {
            client_order_id,
            open_orders,
            open_orders_address,
            mut req_q,
            orders_owner: _,
        } = args;
        let mut slot = None;
        for (i, ith_client_order_id) in open_orders.client_order_ids.iter().enumerate() {
            if *ith_client_order_id == client_order_id.get() && !open_orders.slot_is_free(i as u8) {
                slot = Some(i);
                break;
            }
        }
        let expected_open_orders_slot = slot.ok_or(DexErrorCode::ClientIdNotFound)? as u8;
        let side = open_orders
            .slot_side(expected_open_orders_slot)
            .ok_or(DexErrorCode::ClientIdNotFound)?;
        let ref order_id = open_orders.orders[expected_open_orders_slot as usize];
        let request = Request::new(RequestView::CancelOrder {
            cancel_id: req_q.gen_seq_num(),
            expected_owner: open_orders_address,
            expected_owner_slot: expected_open_orders_slot,
            order_id,
            side,
            client_order_id: Some(client_order_id),
        });
        req_q
            .push_back(request)
            .map_err(|_| DexErrorCode::RequestQueueFull)?;
        Ok(())
    }

    fn process_cancel_order(args: account_parser::CancelOrderArgs) -> DexResult {
        let account_parser::CancelOrderArgs {
            instruction,
            open_orders: _,
            open_orders_address,
            mut req_q,
            orders_owner: _,
        } = args;

        let request = Request::new(RequestView::CancelOrder {
            cancel_id: req_q.gen_seq_num(),
            expected_owner: open_orders_address,
            expected_owner_slot: instruction.owner_slot,
            order_id: &instruction.order_id,
            side: instruction.side,
            client_order_id: None,
        });
        req_q
            .push_back(request)
            .map_err(|_| DexErrorCode::RequestQueueFull)?;
        Ok(())
    }

    fn process_consume_events(args: account_parser::ConsumeEventsArgs) -> DexResult {
        let account_parser::ConsumeEventsArgs {
            limit,
            program_id,
            open_orders_accounts,
            market,
            mut event_q,
        } = args;

        for _i in 0u16..limit {
            let event = match event_q.peek_front() {
                None => break,
                Some(e) => e,
            };

            let view = event.as_view()?;
            let owner: [u64; 4] = event.owner;
            let owner_index: Result<usize, usize> = open_orders_accounts
                .binary_search_by_key(&owner, |account_info| account_info.key.to_aligned_bytes());
            let mut open_orders: RefMut<OpenOrders> = match owner_index {
                Err(_) => break,
                Ok(i) => {
                    market.load_orders_mut(&open_orders_accounts[i], None, program_id, None)?
                }
            };

            check_assert!(event.owner_slot < 128)?;
            check_assert_eq!(&open_orders.slot_side(event.owner_slot), &Some(view.side()))?;
            check_assert_eq!(
                &open_orders.orders[event.owner_slot as usize],
                &event.order_id
            )?;

            match event.as_view()? {
                EventView::Fill {
                    side,
                    maker,
                    native_qty_paid,
                    native_qty_received,
                    native_fee_or_rebate,
                    fee_tier: _,
                    order_id: _,
                    owner: _,
                    owner_slot,
                    client_order_id,
                } => {
                    match side {
                        Side::Bid => {
                            open_orders.native_pc_total -= native_qty_paid;
                            open_orders.native_coin_total += native_qty_received;
                            open_orders.native_coin_free += native_qty_received;

                            if maker {
                                open_orders.native_pc_free += native_fee_or_rebate;
                            }
                        }
                        Side::Ask => {
                            open_orders.native_coin_total -= native_qty_paid;
                            open_orders.native_pc_total += native_qty_received;
                            open_orders.native_pc_free += native_qty_received;
                        }
                    };
                    if !maker {
                        let referrer_rebate = fees::referrer_rebate(native_fee_or_rebate);
                        open_orders.referrer_rebates_accrued += referrer_rebate;
                    }
                    if let Some(client_id) = client_order_id {
                        debug_assert_eq!(
                            client_id.get(),
                            open_orders.client_order_ids[owner_slot as usize]
                        );
                    }
                }
                EventView::Out {
                    side,
                    native_qty_unlocked,
                    native_qty_still_locked,
                    order_id: _,
                    owner: _,
                    owner_slot,
                    client_order_id,
                } => {
                    let fully_out = native_qty_still_locked == 0;

                    match side {
                        Side::Bid => {
                            open_orders.native_pc_free += native_qty_unlocked;
                            check_assert!(
                                open_orders.native_pc_free <= open_orders.native_pc_total
                            )?;
                        }
                        Side::Ask => {
                            open_orders.native_coin_free += native_qty_unlocked;
                            check_assert!(
                                open_orders.native_coin_free <= open_orders.native_coin_total
                            )?;
                        }
                    };
                    if let Some(client_id) = client_order_id {
                        debug_assert_eq!(
                            client_id.get(),
                            open_orders.client_order_ids[owner_slot as usize]
                        );
                    }
                    if fully_out {
                        open_orders.remove_order(owner_slot)?;
                    }
                }
            };

            event_q
                .pop_front()
                .map_err(|()| DexErrorCode::ConsumeEventsQueueFailure)?;
        }
        Ok(())
    }

    fn process_match_orders(args: account_parser::MatchOrdersArgs) -> DexResult {
        let account_parser::MatchOrdersArgs {
            mut order_book_state,
            mut req_q,
            mut event_q,
            limit,
        } = args;
        order_book_state.process_requests(&mut req_q, &mut event_q, limit)
    }

    #[cfg(feature = "program")]
    fn process_new_order(args: account_parser::NewOrderArgs) -> DexResult {
        let account_parser::NewOrderArgs {
            instruction,
            market,
            open_orders,
            open_orders_address,
            mut req_q,
            payer,
            owner,
            coin_vault,
            pc_vault,
            spl_token_program,
            fee_tier,
        } = args;

        let deposit_amount;
        let deposit_vault;

        let native_pc_qty_locked;
        match instruction.side {
            Side::Bid => {
                let lock_qty_lots = instruction
                    .max_qty
                    .get()
                    .checked_mul(instruction.limit_price.get())
                    .ok_or(DexErrorCode::InsufficientFunds)?;
                let native_lock_qty_before_fee = lock_qty_lots
                    .checked_mul(market.pc_lot_size)
                    .ok_or(DexErrorCode::InsufficientFunds)?;
                let lock_qty_native = native_lock_qty_before_fee
                    .checked_add(fee_tier.taker_fee(native_lock_qty_before_fee))
                    .ok_or(DexErrorCode::InsufficientFunds)?;
                native_pc_qty_locked = Some(NonZeroU64::new(lock_qty_native).unwrap());
                let free_qty_to_lock = lock_qty_native.min(open_orders.native_pc_free);
                deposit_amount = lock_qty_native - free_qty_to_lock;
                deposit_vault = pc_vault.token_account();
                open_orders.lock_free_pc(free_qty_to_lock);
                open_orders.credit_locked_pc(deposit_amount);
                market.pc_deposits_total = market
                    .pc_deposits_total
                    .checked_add(deposit_amount)
                    .unwrap();
            }
            Side::Ask => {
                let lock_qty_native = instruction
                    .max_qty
                    .get()
                    .checked_mul((*market).coin_lot_size)
                    .ok_or(DexErrorCode::InsufficientFunds)?;
                let free_qty_to_lock = lock_qty_native.min(open_orders.native_coin_free);
                deposit_amount = lock_qty_native - free_qty_to_lock;
                deposit_vault = coin_vault.token_account();
                open_orders.lock_free_coin(free_qty_to_lock);
                open_orders.credit_locked_coin(deposit_amount);
                native_pc_qty_locked = None;
                market.coin_deposits_total = market
                    .coin_deposits_total
                    .checked_add(deposit_amount)
                    .unwrap();
            }
        };

        // pull balances from payer, signed by owner
        let deposit_instruction = spl_token::instruction::transfer(
            &spl_token::ID,
            payer.inner().key,
            deposit_vault.inner().key,
            owner.inner().key,
            &[],
            deposit_amount,
        )
        .unwrap();
        assert_eq!(*spl_token_program.inner().key, spl_token::ID);

        // pull the deposited funds
        invoke_spl_token(
            &deposit_instruction,
            &[
                payer.inner().clone(),
                deposit_vault.inner().clone(),
                owner.inner().clone(),
                spl_token_program.inner().clone(),
            ],
            &[],
        )
        .map_err(|err| match err {
            ProgramError::Custom(i) => match TokenError::from_u32(i) {
                Some(TokenError::InsufficientFunds) => DexErrorCode::InsufficientFunds,
                _ => DexErrorCode::TransferFailed,
            },
            _ => DexErrorCode::TransferFailed,
        })?;

        // record the open order in the user account
        let order_id = req_q.gen_order_id(instruction.limit_price.get(), instruction.side);
        let owner_slot = open_orders.add_order(order_id, instruction.side)?;
        open_orders.client_order_ids[owner_slot as usize] = instruction.client_id;

        // add the request to the queue
        let request = Request::new(RequestView::NewOrder {
            side: instruction.side,
            order_type: instruction.order_type,
            order_id: &order_id,
            fee_tier,
            self_trade_behavior: instruction.self_trade_behavior,
            owner: open_orders_address,
            owner_slot,
            max_coin_qty: instruction.max_qty,
            native_pc_qty_locked,
            client_order_id: NonZeroU64::new(instruction.client_id),
        });

        req_q
            .push_back(request)
            .map_err(|_| DexErrorCode::RequestQueueFull)?;
        Ok(())
    }

    fn process_disable_market(args: account_parser::DisableMarketArgs) -> DexResult {
        let account_parser::DisableMarketArgs {
            market,
            authorization: _,
        } = args;
        market.account_flags = market.account_flags | (AccountFlag::Disabled as u64);
        Ok(())
    }

    #[cfg(feature = "program")]
    fn process_sweep_fees(args: account_parser::SweepFeesArgs) -> DexResult {
        let account_parser::SweepFeesArgs {
            mut market,
            pc_vault,
            fee_receiver,
            vault_signer,
            spl_token_program,
            authorization: _,
        } = args;
        let token_amount = market.pc_fees_accrued;
        market.pc_fees_accrued = 0;

        let market_pubkey = market.pubkey();
        let vault_signer_seeds = gen_vault_signer_seeds(&market.vault_signer_nonce, &market_pubkey);
        send_from_vault(
            token_amount,
            fee_receiver.token_account(),
            pc_vault.token_account(),
            spl_token_program,
            vault_signer,
            &vault_signer_seeds,
        )
    }

    fn process_initialize_market(args: account_parser::InitializeMarketArgs) -> DexResult {
        let &InitializeMarketInstruction {
            coin_lot_size,
            pc_lot_size,
            fee_rate_bps,
            vault_signer_nonce,
            pc_dust_threshold,
        } = args.instruction;

        let market = args.get_market();
        let req_q = args.get_req_q();
        let event_q = args.get_event_q();
        let bids = args.get_bids();
        let asks = args.get_asks();
        let coin_vault = args.coin_vault_and_mint.get_account().inner();
        let coin_mint = args.coin_vault_and_mint.get_mint().inner();
        let pc_vault = args.pc_vault_and_mint.get_account().inner();
        let pc_mint = args.pc_vault_and_mint.get_mint().inner();

        // initialize request queue
        let mut rq_data = req_q.try_borrow_mut_data()?;
        const RQ_HEADER_WORDS: usize = size_of::<RequestQueueHeader>() / size_of::<u64>();
        let rq_view = init_account_padding(&mut rq_data)?;
        let (rq_hdr_array, rq_buf_words) = mut_array_refs![rq_view, RQ_HEADER_WORDS; .. ;];
        let rq_buf: &[Request] = remove_slop(cast_slice(rq_buf_words));
        if rq_buf.is_empty() {
            Err(DexErrorCode::RequestQueueEmpty)?
        }
        let rq_hdr: &mut RequestQueueHeader =
            try_cast_mut(rq_hdr_array).or(check_unreachable!())?;
        *rq_hdr = RequestQueueHeader {
            account_flags: (AccountFlag::Initialized | AccountFlag::RequestQueue).bits(),
            head: 0,
            count: 0,
            next_seq_num: 0,
        };
        // initialize event queue
        let mut eq_data = event_q.try_borrow_mut_data().unwrap();
        const EQ_HEADER_WORDS: usize = size_of::<EventQueueHeader>() / size_of::<u64>();
        let eq_view = init_account_padding(&mut eq_data)?;
        check_assert!(eq_view.len() > EQ_HEADER_WORDS)?;
        let (eq_hdr_array, eq_buf_words) = mut_array_refs![eq_view, EQ_HEADER_WORDS; .. ;];
        let eq_buf: &[Event] = remove_slop(cast_slice(eq_buf_words));
        if eq_buf.len() < 128 {
            Err(DexErrorCode::EventQueueTooSmall)?
        }
        let eq_hdr: &mut EventQueueHeader = try_cast_mut(eq_hdr_array).or(check_unreachable!())?;
        *eq_hdr = EventQueueHeader {
            account_flags: (AccountFlag::Initialized | AccountFlag::EventQueue).bits(),
            head: 0,
            count: 0,
            seq_num: 0,
        };
        // initialize orderbook storage
        for (flag, account) in &[(AccountFlag::Bids, bids), (AccountFlag::Asks, asks)] {
            let mut ob_data = account.try_borrow_mut_data().unwrap();
            let ob_view = init_account_padding(&mut ob_data)?;
            const OB_HEADER_WORDS: usize = size_of::<OrderBookStateHeader>() / size_of::<u64>();
            check_assert!(ob_view.len() > OB_HEADER_WORDS)?;
            let (hdr_array, slab_words) = mut_array_refs![ob_view, OB_HEADER_WORDS; .. ;];
            let ob_hdr: &mut OrderBookStateHeader =
                try_cast_mut(hdr_array).or(check_unreachable!())?;
            *ob_hdr = OrderBookStateHeader {
                account_flags: (AccountFlag::Initialized | *flag).bits(),
            };
            let slab = Slab::new(cast_slice_mut(slab_words));
            slab.assert_minimum_capacity(100)?;
        }
        info!("Initializing market...");
        // initialize market
        let mut market_data = market.try_borrow_mut_data()?;
        let market_view = init_account_padding(&mut market_data)?;
        let market_hdr: &mut MarketState =
            try_from_bytes_mut(cast_slice_mut(market_view)).or(check_unreachable!())?;
        *market_hdr = MarketState {
            coin_lot_size,
            pc_lot_size,
            own_address: market.key.to_aligned_bytes(),
            account_flags: (AccountFlag::Initialized | AccountFlag::Market).bits(),

            coin_mint: coin_mint.key.to_aligned_bytes(),
            coin_vault: coin_vault.key.to_aligned_bytes(),
            coin_deposits_total: 0,
            coin_fees_accrued: 0,

            req_q: req_q.key.to_aligned_bytes(),
            event_q: event_q.key.to_aligned_bytes(),
            bids: bids.key.to_aligned_bytes(),
            asks: asks.key.to_aligned_bytes(),

            pc_mint: pc_mint.key.to_aligned_bytes(),
            pc_vault: pc_vault.key.to_aligned_bytes(),
            pc_deposits_total: 0,
            pc_fees_accrued: 0,
            vault_signer_nonce,

            pc_dust_threshold,
            fee_rate_bps: fee_rate_bps as u64,
            referrer_rebates_accrued: 0,
        };
        Ok(())
    }
}
