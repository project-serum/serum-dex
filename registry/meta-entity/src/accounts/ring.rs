use crate::error::{MetaEntityError, MetaEntityErrorCode};
use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use borsh::{BorshDeserialize, BorshSerialize};
use serum_common::pack::*;
use solana_client_gen::solana_sdk::account_info::AccountInfo;
use solana_client_gen::solana_sdk::pubkey::Pubkey;
#[cfg(feature = "program")]
use solana_sdk::info;
use std::cell::RefCell;
use std::convert::Into;
use std::rc::Rc;

#[cfg(not(feature = "program"))]
macro_rules! info {
    ($($i:expr),*) => { { ($($i),*) } };
}

// Data storage index at which the messages start.
pub const MESSAGE_START: u32 = 8;

// 280 + 32 + 8.
//
// Ends up being 236 chars wtih borsh overhead.
pub const MESSAGE_SIZE: u32 = 320;

// Max number of messages in the queue before overwriting the tail.
pub const MAX_MESSAGES: u32 = 500;

// Byte size of the entire account.
pub const SIZE: usize = (MESSAGE_SIZE as usize * MAX_MESSAGES as usize) + MESSAGE_START as usize;

pub struct Ring<'a> {
    pub storage: Rc<RefCell<&'a mut [u8]>>,
}

impl<'a> Ring<'a> {
    pub fn from(storage: Rc<RefCell<&'a mut [u8]>>) -> Self {
        Self { storage }
    }

    pub fn append(&self, mut data: Vec<u8>) -> Result<(), MetaEntityError> {
        if data.len() > MESSAGE_SIZE as usize {
            return Err(MetaEntityErrorCode::InvalidMessageSize)?;
        }
        let head = self.head()?;
        let tail = self.tail()?;

        // Scope into a block so that the refcell is dropped.
        {
            let head_idx = (head * MESSAGE_SIZE + MESSAGE_START) as usize;
            let mut acc_data = self.storage.borrow_mut();
            let dst = array_mut_ref![acc_data, head_idx, MESSAGE_SIZE as usize];
            data.resize(MESSAGE_SIZE as usize, 0);
            dst.copy_from_slice(&data);
        }
        // If full, then move the tail as well.
        if (head + 1) % MAX_MESSAGES == tail {
            self.increment_tail();
        }
        self.increment_head();

        Ok(())
    }

    pub fn messages_rev(&self) -> Result<Vec<Message>, MetaEntityError> {
        let data = self.storage.borrow();
        let head = self.head()?;
        let tail = self.tail()?;

        // Empty.
        if head == tail {
            return Ok(vec![]);
        }

        let mut msgs = vec![];
        let mut last = head;
        while tail != last {
            if last == 0 {
                last = MAX_MESSAGES - 1;
            } else {
                last -= 1;
            }

            let start = (last * MESSAGE_SIZE + MESSAGE_START) as usize;
            let end = start + MESSAGE_SIZE as usize;

            let m = Message::unpack_unchecked(&mut &data[start..end])?;
            msgs.push(m);
        }

        Ok(msgs)
    }

    pub fn message_at(&self, cursor: u32) -> Result<Message, MetaEntityError> {
        let data = self.storage.borrow();
        let mut dst = [0u8; MESSAGE_SIZE as usize];
        let start = (MESSAGE_START + cursor * MESSAGE_SIZE) as usize;
        let end = start + MESSAGE_SIZE as usize;
        dst.copy_from_slice(&data[start..end]);
        let mut dst_slice: &[u8] = &dst;
        Message::unpack_unchecked(&mut dst_slice).map_err(Into::into)
    }

    fn head(&self) -> Result<u32, MetaEntityError> {
        let data = self.storage.borrow();
        let mut dst = [0u8; 4];
        dst.copy_from_slice(&data[..4]);
        Ok(u32::from_le_bytes(dst))
    }

    fn tail(&self) -> Result<u32, MetaEntityError> {
        let data = self.storage.borrow();
        let mut dst = [0u8; 4];
        dst.copy_from_slice(&data[4..8]);
        Ok(u32::from_le_bytes(dst))
    }

    fn increment_head(&self) -> Result<(), MetaEntityError> {
        let mut head = self.head()?;
        if head == MAX_MESSAGES - 1 {
            head = 0;
        } else {
            head += 1;
        }
        self.set_head(head)?;
        Ok(())
    }

    fn increment_tail(&self) -> Result<(), MetaEntityError> {
        let mut tail = self.tail()?;
        if tail == MAX_MESSAGES - 1 {
            tail = 0;
        } else {
            tail += 1;
        }
        self.set_tail(tail)?;
        Ok(())
    }

    fn set_head(&self, head: u32) -> Result<(), MetaEntityError> {
        let mut data = self.storage.borrow_mut();
        let dst = array_mut_ref![data, 0, 4];
        dst.copy_from_slice(&head.to_le_bytes());
        Ok(())
    }

    fn set_tail(&self, tail: u32) -> Result<(), MetaEntityError> {
        let mut data = self.storage.borrow_mut();
        let dst = array_mut_ref![data, 4, 4];
        dst.copy_from_slice(&tail.to_le_bytes());
        Ok(())
    }
}

#[derive(Debug, BorshSerialize, BorshDeserialize, PartialEq)]
pub struct Message {
    from: Pubkey,
    ts: i64,
    content: String,
}

serum_common::packable!(Message);

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mqueue() {
        let mut data: &mut [u8] = &mut vec![0u8; SIZE];
        let storage = Rc::new(RefCell::new(data));
        let mqueue = from(storage);
        let mut messages = vec![];

        // First pass: fill the message queue.
        for k in 0u32..MAX_MESSAGES - 1 {
            let m = Message {
                from: Pubkey::new_rand(),
                ts: k as i64,
                content: "hello world".to_string(),
            };
            mqueue.append(m.try_to_vec().unwrap());
            messages.insert(0, m);

            assert_eq!(mqueue.messages_rev().unwrap(), messages);
            assert_eq!(mqueue.tail().unwrap(), 0);
            assert_eq!(mqueue.head().unwrap(), k + 1);
        }

        // Buffer is now full. Adding more will overwrite previous messages.
        // Head is always one behind the tail now, so technically we waste
        // a slot.
        assert_eq!(mqueue.tail().unwrap(), 0);
        assert_eq!(mqueue.head().unwrap(), MAX_MESSAGES - 1);

        // Insert one to begin the wrap.
        let m = Message {
            from: Pubkey::new_rand(),
            ts: 0,
            content: "hello world".to_string(),
        };
        mqueue.append(m.try_to_vec().unwrap());
        messages.pop();
        messages.insert(0, m);
        assert_eq!(mqueue.messages_rev().unwrap(), messages);
        assert_eq!(mqueue.tail().unwrap(), 1);
        assert_eq!(mqueue.head().unwrap(), 0);

        // Do another pass, overwriting all previous messages.
        for k in 0u32..MAX_MESSAGES {
            let m = Message {
                from: Pubkey::new_rand(),
                ts: k as i64,
                content: "hello world".to_string(),
            };
            mqueue.append(m.try_to_vec().unwrap());
            messages.pop();
            messages.insert(0, m);
            assert_eq!(mqueue.messages_rev().unwrap(), messages);
            assert_eq!(mqueue.tail().unwrap(), (k + 2) % MAX_MESSAGES);
            assert_eq!(mqueue.head().unwrap(), (k + 1) % MAX_MESSAGES);
        }

        // Back where we started.
        assert_eq!(mqueue.tail().unwrap(), 1);
        assert_eq!(mqueue.head().unwrap(), 0);
    }
}
