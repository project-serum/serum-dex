use crate::accounts::entity::PoolPrices;
use borsh::{BorshDeserialize, BorshSchema, BorshSerialize};
use serum_common::pack::*;
use solana_client_gen::solana_sdk::pubkey::Pubkey;

/// A Generation account stores the staking pool price for a given node Entity
/// as of a given activation generation--marked to the last invocation of the
/// `MarkGeneration` instruction.
///
/// Because staking rewards cease once a node entity's total stake deposit
/// falls below a threshold (e.g. 1 MSRM), this is used to price a staking
/// pool token in the event of withdrawing from an *inactive* node.
///
/// When withdrawing from an entity with state "active" or "pending-deactivation"
/// the current pool prices will be used (and so this account is not needed).
///
/// In the event of node deactivation, it's expected good citizens of a node
/// will publicly publish these Generation accounts so that members forgetting
/// to withdraw before deactivation can retrieve their full rewards. Otherwise,
/// their pool token will be marked to the price of the last time they staked,
/// which may be lower than the price upon deactivation (since rewards can be
/// dropped onto the pool after staking).
#[derive(Default, Debug, BorshSerialize, BorshDeserialize, BorshSchema)]
pub struct Generation {
    pub initialized: bool,
    pub entity: Pubkey,
    pub generation: u64,
    pub last_active_prices: PoolPrices,
}

serum_common::packable!(Generation);
