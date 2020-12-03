use borsh::{BorshDeserialize, BorshSchema, BorshSerialize};
use serum_common::pack::*;
use solana_client_gen::solana_sdk::pubkey::Pubkey;
use std::cell::RefCell;
use std::rc::Rc;

// Size of each entry in the ring buffer.
const RING_ITEM_SIZE: u32 = 320;

// Generate the Ring trait.
serum_common::ring!(RING_ITEM_SIZE);

pub struct MQueue<'a> {
    pub storage: Rc<RefCell<&'a mut [u8]>>,
}

impl<'a> MQueue<'a> {
    pub const RING_CAPACITY: u32 = 500;

    pub fn from(storage: Rc<RefCell<&'a mut [u8]>>) -> Self {
        Self { storage }
    }
}

impl<'a> Ring<'a> for MQueue<'a> {
    type Item = Message;

    fn buffer(&self) -> Rc<RefCell<&'a mut [u8]>> {
        self.storage.clone()
    }
    fn capacity(&self) -> u32 {
        MQueue::RING_CAPACITY
    }
}

#[derive(Debug, BorshSerialize, BorshDeserialize, BorshSchema, PartialEq)]
pub struct Message {
    from: Pubkey,
    ts: i64,
    content: String,
}

serum_common::packable!(Message);
