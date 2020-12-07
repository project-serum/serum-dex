// Macro to generate an append only Ring buffer trait. Clients should use this
// to create ring buffer accounts, defining a fixed ITEM_SIZE, the maximum
// size for each entry in the ring.
//
// Uses a macro to define the trait since  Rust doesn't (yet) support arrays
// with generic lengths.
//
// Remove macro (and just use the trait) once the following issue is addressed:
// https://github.com/rust-lang/rust/issues/43408
#[macro_export]
macro_rules! ring {
    ($message_size:expr) => {
        use arrayref::array_mut_ref;

        const AUTHORITY_START: usize = 0;
        // TODO: bump head + tail to be u64 instead of u32.
        const HEAD_START: usize = 32;
        const TAIL_START: usize = 36;
        const ITEM_START: u32 = 40;

        pub trait Ring<'a> {
            type Item: BorshSerialize + BorshDeserialize;

            const ITEM_SIZE: u32 = $message_size;

            fn buffer(&self) -> Rc<RefCell<&'a mut [u8]>>;
            fn capacity(&self) -> u32;

            fn buffer_size(capacity: u32) -> usize {
                (Self::ITEM_SIZE as usize * capacity as usize) + ITEM_START as usize
            }

            fn append(&self, item: &Self::Item) -> Result<(), ProgramError> {
                let mut data = item
                    .try_to_vec()
                    .map_err(|_| ProgramError::InvalidAccountData)?;

                if data.len() > Self::ITEM_SIZE as usize {
                    return Err(ProgramError::InvalidAccountData)?;
                }
                let head = self.head()?;
                let tail = self.tail()?;

                // Scope into a block so that the refcell is dropped.
                {
                    let head_idx = (head * Self::ITEM_SIZE + ITEM_START) as usize;
                    let buffer = self.buffer();
                    let mut acc_data = buffer.borrow_mut();
                    let dst = array_mut_ref![acc_data, head_idx, $message_size as usize];
                    data.resize(Self::ITEM_SIZE as usize, 0);
                    dst.copy_from_slice(&data);
                }
                // If full, then move the tail as well.
                if (head + 1) % self.capacity() == tail {
                    self.increment_tail()?;
                }
                self.increment_head()?;

                Ok(())
            }

            #[cfg(not(feature = "program"))]
            fn messages_rev(&self) -> Result<Vec<Self::Item>, ProgramError> {
                let buffer = self.buffer();
                let data = buffer.borrow();
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
                        last = self.capacity() - 1;
                    } else {
                        last -= 1;
                    }

                    let start = (last * Self::ITEM_SIZE + ITEM_START) as usize;
                    let end = start + Self::ITEM_SIZE as usize;

                    let m = Self::Item::deserialize(&mut &data[start..end])
                        .map_err(|_| ProgramError::InvalidAccountData)?;
                    msgs.push(m);
                }

                Ok(msgs)
            }

            fn authority(&self) -> Pubkey {
                let buffer = self.buffer();
                let data = buffer.borrow();
                let mut dst = [0u8; 32];
                dst.copy_from_slice(&data[AUTHORITY_START..AUTHORITY_START + 32]);
                Pubkey::new_from_array(dst)
            }

            fn set_authority(&self, authority: &Pubkey) {
                let buffer = self.buffer();
                let mut data = buffer.borrow_mut();
                let dst = array_mut_ref![data, AUTHORITY_START, 32];
                dst.copy_from_slice(authority.as_ref());
            }

            fn message_at(&self, cursor: u32) -> Result<Self::Item, ProgramError> {
                let index = cursor % self.capacity();
                let buffer = self.buffer();
                let data = buffer.borrow();
                let mut dst = vec![0u8; Self::ITEM_SIZE as usize];
                let start = (ITEM_START + index * Self::ITEM_SIZE) as usize;
                let end = start + Self::ITEM_SIZE as usize;
                dst.copy_from_slice(&data[start..end]);
                Self::Item::deserialize(&mut dst.as_ref())
                    .map_err(|_| ProgramError::InvalidAccountData)
            }

            // Head is the next available position in the ring buffer for
            // appending.
            fn head(&self) -> Result<u32, ProgramError> {
                Ok(self.head_cursor()? % self.capacity())
            }

            fn head_cursor(&self) -> Result<u32, ProgramError> {
                let buffer = self.buffer();
                let data = buffer.borrow();
                let mut dst = [0u8; 4];
                dst.copy_from_slice(&data[HEAD_START..HEAD_START + 4]);
                Ok(u32::from_le_bytes(dst))
            }

            // Tail is the first taken position in the ring buffer,
            // except when tail === head. In which case the buffer is empty.
            // When the buffer is full, tail == head + 1.
            fn tail(&self) -> Result<u32, ProgramError> {
                Ok(self.tail_cursor()? % self.capacity())
            }

            fn tail_cursor(&self) -> Result<u32, ProgramError> {
                let buffer = self.buffer();
                let data = buffer.borrow();
                let mut dst = [0u8; 4];
                dst.copy_from_slice(&data[TAIL_START..TAIL_START + 4]);
                Ok(u32::from_le_bytes(dst))
            }

            fn increment_head(&self) -> Result<(), ProgramError> {
                let head = self.head_cursor()?;
                self.set_head(head + 1)?;
                Ok(())
            }

            fn increment_tail(&self) -> Result<(), ProgramError> {
                let tail = self.tail_cursor()?;
                self.set_tail(tail + 1)?;
                Ok(())
            }

            fn set_head(&self, head: u32) -> Result<(), ProgramError> {
                let buffer = self.buffer();
                let mut data = buffer.borrow_mut();
                let dst = array_mut_ref![data, HEAD_START, 4];
                dst.copy_from_slice(&head.to_le_bytes());
                Ok(())
            }

            fn set_tail(&self, tail: u32) -> Result<(), ProgramError> {
                let buffer = self.buffer();
                let mut data = buffer.borrow_mut();
                let dst = array_mut_ref![data, TAIL_START, 4];
                dst.copy_from_slice(&tail.to_le_bytes());
                Ok(())
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use crate as serum_common;
    use crate::pack::*;
    use borsh::{BorshDeserialize, BorshSerialize};
    use solana_sdk::pubkey::Pubkey;
    use std::cell::RefCell;
    use std::rc::Rc;

    ring!(320);

    struct MQueue<'a> {
        pub storage: Rc<RefCell<&'a mut [u8]>>,
    }

    impl<'a> MQueue<'a> {
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
            500
        }
    }

    #[derive(Debug, Clone, BorshSerialize, BorshDeserialize, PartialEq)]
    struct Message {
        from: Pubkey,
        ts: i64,
        content: String,
    }

    packable!(Message);

    #[test]
    fn mqueue() {
        let data: &mut [u8] = &mut vec![0u8; MQueue::buffer_size(500)];
        let storage = Rc::new(RefCell::new(data));
        let mqueue = MQueue::from(storage);

        let authority = Pubkey::new_unique();
        mqueue.set_authority(&authority);
        assert_eq!(authority, mqueue.authority());

        let mut messages = vec![];

        // First pass: fill the message queue.
        for k in 0u32..mqueue.capacity() - 1 {
            let m = Message {
                from: Pubkey::new_unique(),
                ts: k as i64,
                content: "hello world".to_string(),
            };
            mqueue.append(&m).unwrap();
            messages.insert(0, m);

            assert_eq!(mqueue.messages_rev().unwrap(), messages);
            assert_eq!(mqueue.tail().unwrap(), 0);
            assert_eq!(mqueue.head().unwrap(), k + 1);
        }

        // Buffer is now full. Adding more will overwrite previous messages.
        // Head is always one behind the tail now, so technically we waste
        // a slot.
        assert_eq!(mqueue.tail().unwrap(), 0);
        assert_eq!(mqueue.head().unwrap(), mqueue.capacity() - 1);

        // Insert one to begin the wrap.
        let m = Message {
            from: Pubkey::new_unique(),
            ts: 0,
            content: "hello world".to_string(),
        };
        mqueue.append(&m).unwrap();
        messages.pop();
        messages.insert(0, m);
        assert_eq!(mqueue.messages_rev().unwrap(), messages);
        assert_eq!(mqueue.tail().unwrap(), 1);
        assert_eq!(mqueue.head().unwrap(), 0);

        // Do another pass, overwriting all previous messages.
        for k in 0u32..mqueue.capacity() {
            let m = Message {
                from: Pubkey::new_unique(),
                ts: k as i64,
                content: "hello world".to_string(),
            };
            mqueue.append(&m).unwrap();
            messages.pop();
            messages.insert(0, m);
            assert_eq!(mqueue.messages_rev().unwrap(), messages);
            assert_eq!(mqueue.tail().unwrap(), (k + 2) % mqueue.capacity());
            assert_eq!(mqueue.head().unwrap(), (k + 1) % mqueue.capacity());
        }

        // Back where we started.
        assert_eq!(mqueue.tail().unwrap(), 1);
        assert_eq!(mqueue.head().unwrap(), 0);
    }
}
