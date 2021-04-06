use crate::matching::{OrderBookState, Side};
use crate::error::DexResult;
use solana_program::{
    pubkey::Pubkey,
    msg
};
use crate::critbit::{SlabView, AnyNode, LeafNode};

impl<'ob> OrderBookState<'ob> {
    pub fn prune(
        &mut self,
        gateway_token: [u64; 4],
    ) -> DexResult<usize> {
        let asks_matching_gateway_token = self.asks.find_by(|order| order.gateway_token().eq(&gateway_token) );
        let bids_matching_gateway_token = self.bids.find_by(|order| order.gateway_token().eq(&gateway_token) );
        let asks_removed: Vec<Option<LeafNode>> = asks_matching_gateway_token
            .iter()
            .map(|order_to_remove| self.asks.remove_by_key(*order_to_remove))
            .filter(| node | node.is_some())
            .collect();
        let bids_removed: Vec<Option<LeafNode>> = bids_matching_gateway_token
            .iter()
            .map(|order_to_remove| self.bids.remove_by_key(*order_to_remove))
            .filter(| node | node.is_some())
            .collect();
        
        msg!("Pruned {} asks and {} bids", asks_removed.len(), bids_removed.len());
        
        Ok(asks_removed.len() + bids_removed.len())
    }
}