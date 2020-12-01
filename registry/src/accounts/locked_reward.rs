pub struct LockedReward {
    pub vault: Pubkey,
    pub nonce: u8,
    pub total: u64,
    pub start: i64,
    pub end: i64,
    pub pool: Pubkey,
    // Supply of the pool token at time of reward allocation.
    pub pool_token_supply: u64,
}
