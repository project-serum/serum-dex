use borsh::{BorshDeserialize, BorshSerialize};
use serum_common::pack::*;
use solana_client_gen::solana_sdk::pubkey::Pubkey;
use std::cell::RefCell;
use std::rc::Rc;

// Largest reward variant size.
//
// Don't forget to change the typescript when modifying this.
const MAX_RING_ITEM_SIZE: u32 = 145;

// Generate the Ring trait.
serum_common::ring!(MAX_RING_ITEM_SIZE);

pub struct RewardEventQueue<'a> {
    pub storage: Rc<RefCell<&'a mut [u8]>>,
}

impl<'a> RewardEventQueue<'a> {
    // Don't forget to change the typescript when modifying this.
    pub const RING_CAPACITY: u32 = 13792;

    pub fn from(storage: Rc<RefCell<&'a mut [u8]>>) -> Self {
        Self { storage }
    }
}

impl<'a> Ring<'a> for RewardEventQueue<'a> {
    type Item = RewardEvent;

    fn buffer(&self) -> Rc<RefCell<&'a mut [u8]>> {
        self.storage.clone()
    }
    fn capacity(&self) -> u32 {
        RewardEventQueue::RING_CAPACITY
    }
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub enum RewardEvent {
    LockedAlloc {
        from: Pubkey,
        total: u64,
        pool: Pubkey,
        vendor: Pubkey,
        mint: Pubkey,
        ts: i64,
    },
    UnlockedAlloc {
        from: Pubkey,
        total: u64,
        pool: Pubkey,
        vendor: Pubkey,
        mint: Pubkey,
        ts: i64,
    },
}

serum_common::packable!(RewardEvent);

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn measure() {
        let e = RewardEvent::LockedAlloc {
            from: Pubkey::new_unique(),
            total: 0,
            pool: Pubkey::new_unique(),
            vendor: Pubkey::new_unique(),
            mint: Pubkey::new_unique(),
            ts: 0,
        };
        assert_eq!(e.try_to_vec().unwrap().len(), 145);
    }
}
