use crate::error::{RegistryError, RegistryErrorCode};
use arrayref::array_mut_ref;
use borsh::{BorshDeserialize, BorshSerialize};
use serum_common::pack::*;
use solana_client_gen::solana_sdk::pubkey::Pubkey;
use std::cell::RefCell;
use std::convert::Into;
use std::rc::Rc;

// Largest reward variant size.
const MAX_RING_ITEM_SIZE: u32 = 137;

// Generate the Ring trait.
crate::ring!(MAX_RING_ITEM_SIZE);

pub struct RewardEventQueue<'a> {
    pub storage: Rc<RefCell<&'a mut [u8]>>,
}

impl<'a> RewardEventQueue<'a> {
    pub const RING_CAPACITY: u32 = 14598;

    pub fn from(storage: Rc<RefCell<&'a mut [u8]>>) -> Self {
        Self { storage }
    }
}

impl<'a> Ring<'a, RewardEvent> for RewardEventQueue<'a> {
    fn buffer(&self) -> Rc<RefCell<&'a mut [u8]>> {
        self.storage.clone()
    }
    fn capacity(&self) -> u32 {
        RewardEventQueue::RING_CAPACITY
    }
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub enum RewardEvent {
    // Rewards transferred directly to the pool's vault.
    //
    // Amounts must align with the pool's basket "quantity".
    PoolDrop {
        from: Pubkey,
        totals: Vec<u64>,
        pool: Pubkey,
    },
    LockedAlloc {
        from: Pubkey,
        total: u64,
        pool: Pubkey,
        locked_vendor: Pubkey,
        mint: Pubkey,
    },
}

serum_common::packable!(RewardEvent);

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn measure() {
        let e = RewardEvent::LockedAlloc {
            from: Pubkey::new_rand(),
            total: 0,
            pool: Pubkey::new_rand(),
            locked_vendor: Pubkey::new_rand(),
            mint: Pubkey::new_rand(),
        };
        println!("TEST: {:?}", e.try_to_vec().unwrap().len());
    }
}
