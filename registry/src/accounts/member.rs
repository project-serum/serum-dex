use borsh::{BorshDeserialize, BorshSchema, BorshSerialize};
use serum_common::pack::*;
use solana_client_gen::solana_sdk::pubkey::Pubkey;

#[cfg(feature = "client")]
lazy_static::lazy_static! {
    pub static ref SIZE: u64 = Member::default()
                .size()
                .expect("Vesting has a fixed size");
}

/// Member account tracks membership with a node `Entity`.
#[derive(Default, Debug, BorshSerialize, BorshDeserialize, BorshSchema)]
pub struct Member {
    /// Set by the program on creation.
    pub initialized: bool,
    /// Registrar the member belongs to.
    pub registrar: Pubkey,
    /// Entity account providing membership.
    pub entity: Pubkey,
    /// The key that is allowed to redeem assets from the staking pool.
    pub beneficiary: Pubkey,
    /// The entity's activation id to which the stake belongs.
    pub generation: u64,
    /// The Watchtower that can withdraw the `Member` account's `main` `Book`.
    pub watchtower: Watchtower,
    /// The balance subbaccounts that partition the Member's stake balance.
    pub books: MemberBooks,
}

impl Member {
    pub fn add_stake_intent(&mut self, amount: u64, mega: bool, delegate: bool) {
        if delegate {
            if mega {
                self.books.delegate.balances.mega_stake_intent += amount;
            } else {
                self.books.delegate.balances.stake_intent += amount;
            }
        } else {
            if mega {
                self.books.main.balances.mega_stake_intent += amount;
            } else {
                self.books.main.balances.stake_intent += amount;
            }
        }
    }
    pub fn sub_stake_intent(&mut self, amount: u64, mega: bool, delegate: bool) {
        if delegate {
            if mega {
                self.books.delegate.balances.mega_stake_intent -= amount;
            } else {
                self.books.delegate.balances.stake_intent -= amount;
            }
        } else {
            if mega {
                self.books.main.balances.mega_stake_intent -= amount;
            } else {
                self.books.main.balances.stake_intent -= amount;
            }
        }
    }
    pub fn add_stake(&mut self, amount: u64, mega: bool, delegate: bool) {
        if delegate {
            if mega {
                self.books.delegate.balances.mega_amount += amount;
            } else {
                self.books.delegate.balances.amount += amount;
            }
        } else {
            if mega {
                self.books.main.balances.mega_amount += amount;
            } else {
                self.books.main.balances.amount += amount;
            }
        }
    }
    pub fn transfer_pending_withdrawal(&mut self, amount: u64, mega: bool, delegate: bool) {
        if delegate {
            if mega {
                self.books.delegate.balances.mega_amount -= amount;
                self.books.delegate.balances.mega_pending_withdrawals += amount;
            } else {
                self.books.delegate.balances.amount -= amount;
                self.books.delegate.balances.pending_withdrawals += amount;
            }
        } else {
            if mega {
                self.books.main.balances.mega_amount -= amount;
                self.books.main.balances.mega_pending_withdrawals += amount;
            } else {
                self.books.main.balances.amount -= amount;
                self.books.main.balances.pending_withdrawals += amount;
            }
        }
    }
    pub fn stake_is_empty(&self) -> bool {
        self.books.main.balances.amount != 0
            || self.books.main.balances.mega_amount != 0
            || self.books.delegate.balances.amount != 0
            || self.books.delegate.balances.mega_amount != 0
    }
    pub fn set_delegate(&mut self, delegate: Pubkey) {
        assert!(self.books.delegate.balances.amount == 0);
        self.books.delegate = Book {
            owner: delegate,
            balances: Default::default(),
        };
    }
    pub fn stake_intent(&self, mega: bool, delegate: bool) -> u64 {
        if delegate {
            if mega {
                self.books.delegate.balances.mega_stake_intent
            } else {
                self.books.delegate.balances.stake_intent
            }
        } else {
            if mega {
                self.books.main.balances.mega_stake_intent
            } else {
                self.books.main.balances.stake_intent
            }
        }
    }
}

/// Watchtower defines an (optional) authority that can update a Member account
/// on behalf of the `beneficiary`.
#[derive(Default, Clone, Copy, Debug, BorshSerialize, BorshDeserialize, BorshSchema)]
pub struct Watchtower {
    /// The signing key that can withdraw stake from this Member account in
    /// the case of a pending deactivation.
    authority: Pubkey,
    /// The destination *token* address the staked funds are sent to in the
    /// case of a withdrawal by a watchtower.
    ///
    /// Note that a watchtower can only withdraw deposits *not* sent from a
    /// delegate. Withdrawing more will result in tx failure.
    ///
    /// For all delegated funds, the watchtower should follow the protocol
    /// defined by the delegate.
    ///
    /// In the case of locked SRM, this means invoking the `WhitelistDeposit`
    /// instruction on the Serum Lockup program to transfer funds from the
    /// Registry back into the Lockup.
    dst: Pubkey,
}

impl Watchtower {
    pub fn new(authority: Pubkey, dst: Pubkey) -> Self {
        Self { authority, dst }
    }
}

#[derive(Default, Debug, BorshSerialize, BorshDeserialize, BorshSchema)]
pub struct MemberBooks {
    main: Book,
    /// Delegate authorized to deposit or withdraw from the staking pool
    /// on behalf of the beneficiary up to the `delegate_amount`. Although
    /// these funds are part of the Member account, they are not directly
    /// accessible by the beneficiary. All transactions affecting the delegate
    /// amount has to be signed by the `delegate` key.
    ///
    /// The only expected use case as of now is the Lockup program.
    delegate: Book,
}

impl MemberBooks {
    pub fn new(beneficiary: Pubkey, delegate: Pubkey) -> Self {
        Self {
            main: Book {
                owner: beneficiary,
                balances: Default::default(),
            },
            delegate: Book {
                owner: delegate,
                balances: Default::default(),
            },
        }
    }

    pub fn delegate(&self) -> &Book {
        &self.delegate
    }

    pub fn main(&self) -> &Book {
        &self.main
    }
}

#[derive(Default, Debug, BorshSerialize, BorshDeserialize, BorshSchema)]
pub struct Book {
    pub owner: Pubkey,
    pub balances: Balances,
}

#[derive(Default, Debug, BorshSerialize, BorshDeserialize, BorshSchema)]
pub struct Balances {
    pub amount: u64,
    pub mega_amount: u64,
    pub stake_intent: u64,
    pub mega_stake_intent: u64,
    pub pending_withdrawals: u64,
    pub mega_pending_withdrawals: u64,
}

impl Balances {
    pub fn is_empty(&self) -> bool {
        self.amount
            + self.mega_amount
            + self.stake_intent
            + self.mega_stake_intent
            + self.pending_withdrawals
            + self.mega_pending_withdrawals
            == 0
    }
}

serum_common::packable!(Member);
