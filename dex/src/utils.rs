use std::iter::Peekable;

use crate::{
    critbit::{LeafNodeIterator, Slab},
    matching::Side,
};

pub struct OrderBook<'a> {
    side: Side,
    slab: &'a Slab,
}

impl<'a> OrderBook<'a> {
    pub fn new(side: Side, slab: &'a Slab) -> Self {
        OrderBook { side, slab }
    }

    /// Iterate over level 2 data
    pub fn levels(&'a self) -> LevelsIterator<'a> {
        let is_descending = self.side == Side::Bid;
        LevelsIterator {
            leafs: self.slab.iter(is_descending).peekable(),
        }
    }
}

/// Level 2 data
///
/// Values are in lot sizes.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Level {
    pub price: u64,
    pub quantity: u64,
}

pub struct LevelsIterator<'a> {
    leafs: Peekable<LeafNodeIterator<'a>>,
}

impl Iterator for LevelsIterator<'_> {
    type Item = Level;

    fn next(&mut self) -> Option<Self::Item> {
        let mut level: Option<Level> = None;
        while let Some(leaf) = self.leafs.peek() {
            let price = leaf.price().get();
            let quantity = leaf.quantity();
            match &mut level {
                Some(Level {
                    price: cur_price,
                    quantity: cur_quantity,
                }) => {
                    if price == *cur_price {
                        *cur_quantity += quantity;
                    } else {
                        return level;
                    }
                }
                None => {
                    level = Some(Level { price, quantity });
                }
            }
            self.leafs.next();
        }
        level
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{critbit::LeafNode, fees::FeeTier};

    use bytemuck::cast_slice_mut;
    use rand::prelude::*;

    #[test]
    fn test_levels() {
        let mut aligned_buf = vec![0u64; 10_000];
        let bytes: &mut [u8] = cast_slice_mut(aligned_buf.as_mut_slice());
        let slab = Slab::new(bytes);

        let mut rng = StdRng::from_entropy();

        let orders: Vec<(u64, u64)> = vec![
            (1000, 50),
            (1000, 25),
            (1300, 100),
            (1300, 40),
            (1300, 250),
            (1480, 36),
            (1495, 3000),
            (1495, 340),
        ];
        for (i, (price, qty)) in orders.into_iter().enumerate() {
            let offset = rng.gen();
            let owner = rng.gen();
            let key = ((price as u128) << 64) | (!(i as u64) as u128);

            slab.insert_leaf(&LeafNode::new(offset, key, owner, qty, FeeTier::Base, 0))
                .unwrap();
        }

        let ask_levels = vec![
            Level {
                price: 1000,
                quantity: 75,
            },
            Level {
                price: 1300,
                quantity: 390,
            },
            Level {
                price: 1480,
                quantity: 36,
            },
            Level {
                price: 1495,
                quantity: 3340,
            },
        ];

        let orderbook_asks = OrderBook::new(Side::Ask, slab);
        assert!(ask_levels.clone().into_iter().eq(orderbook_asks.levels()));

        let orderbook_bids = OrderBook::new(Side::Bid, slab);
        assert!(ask_levels.into_iter().rev().eq(orderbook_bids.levels()));
    }
}
