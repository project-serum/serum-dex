use borsh::{BorshDeserialize, BorshSerialize};
use serum_common::pack::*;
use solana_client_gen::solana_sdk::pubkey::Pubkey;

#[cfg(feature = "client")]
lazy_static::lazy_static! {
    pub static ref SIZE: u64 = Member::default()
                .size()
                .expect("Member has a fixed size");
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct Member {
    /// Set by the program on creation.
    pub initialized: bool,
    /// Registrar the member belongs to.
    pub registrar: Pubkey,
    /// The effective owner of the Member account.
    pub beneficiary: Pubkey,
    /// Entity providing membership.
    pub entity: Pubkey,
    /// Arbitrary metadata account owned by any program.
    pub metadata: Pubkey,
    /// Sets of balances owned by the Member. Two for now: main and locked.
    pub balances: Vec<BalanceSandbox>,
    /// Next position in the rewards event queue to process.
    pub rewards_cursor: u32,
    /// The clock timestamp of the last time this account staked or switched
    /// entities.
    // TODO: For v2 we should keep a queue tracking each time the member staked
    //       or unstaked. Then reward vendors can deduce the amount members
    //       had staked at time of reward. For now, we use the last_stake_ts
    //       as an overly harsh mechanism for ensuring rewards are only
    //       given to those that were staked at the right time.
    pub last_stake_ts: i64,
}

impl Default for Member {
    fn default() -> Member {
        Member {
            initialized: false,
            registrar: Pubkey::new_from_array([0; 32]),
            beneficiary: Pubkey::new_from_array([0; 32]),
            entity: Pubkey::new_from_array([0; 32]),
            metadata: Pubkey::new_from_array([0; 32]),
            balances: vec![BalanceSandbox::default(), BalanceSandbox::default()],
            rewards_cursor: 0,
            last_stake_ts: 0,
        }
    }
}

// BalanceSandbox defines isolated funds that can only be deposited/withdrawn
// into the program if the `owner` signs off on the transaction.
//
// Once controlled by the program, the associated `Member` account's beneficiary
// can send funds to/from any of the accounts within the sandbox, e.g., to
// stake.
#[derive(Default, Debug, BorshSerialize, BorshDeserialize)]
pub struct BalanceSandbox {
    pub owner: Pubkey,
    // Staking pool token.
    pub spt: Pubkey,
    pub spt_mega: Pubkey,
    // Free balance (deposit) vaults.
    pub vault: Pubkey,
    pub vault_mega: Pubkey,
    // Stake vaults.
    pub vault_stake: Pubkey,
    pub vault_stake_mega: Pubkey,
    // Pending withdrawal vaults.
    pub vault_pending_withdrawal: Pubkey,
    pub vault_pending_withdrawal_mega: Pubkey,
}

serum_common::packable!(Member);
