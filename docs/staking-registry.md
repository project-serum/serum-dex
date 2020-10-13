# Summary
[summary]: #summary

Defines the Serum Registry program for nodes and staking.

# Design
[design]: #design

## Introduction

The Registry program is the central point of on-chain coordination for Serum nodes
providing two features: a gateway to the staking pool and a respository for node state.

As a gateway to the staking pool, the Registry
controls who and when stakers can deposit and redeem tokens.
This enables features like a mandatory 1 MSRM node deposit before entering
the pool, the staking of locked SRM, and a 1 week timelock for withdrawals.

In addition to providing a gateway to the staking pool, the Registry acts as a respository
for node state, allowing programs to perform access control on nodes. This is useful
for programs paying out rewards for node duties or for those providing governance.
For example, a **crank-rewards** program could use Registry accounts to determine if
a node-leader is eligble for payment (by having 1 MSRM staked to its node) and
what the rate of the reward should be. A **governance** program could tally votes
from node member accounts based on their stake weight and the node's "stake-kind"
(voting or delegated).

In short, the Registry allows nodes to be created, members to stake, and rewards to
be generated, while providing a foundation for other programs to build on.

## Earning Rewards

There are two sources of rewards for Serum Nodes: staking and capabilities.

Staking is rewarded via value accrual on a staking pool token.
Capabilities are rewarded similar to miners by earning what are *effectively*
transaction fees.

### Staking

Staking rewards are provided in the form of a staking pool token.

When a node member deposits SRM (or MSRM) into a pool, the depositor
receives a receipt, i.e. a staking pool token (`spt`), proving the deposit. This
`spt` is an SPL token in and of itself representing a right to the tokens in the
underlying pool. It can be traded, burned, or ultimately exchanged for the proportional
share to the underlying pool.

This allows for staking deposits to accrue value in two natural ways. One is via buy
and burn: one can buy these `spt` tokens on the market and `burn` them, thereby
increasing the amount of tokens in the backing liquidity pool that each `spt` can be
exchanged for. Another is via staking rewards: one can deposit funds directly into
the backing pool, increasing the proportional right to the backing pool shares
in a way similar to buy and burn.

A natural question to ask is, who performs these two types of transactions to increase
the value of the `spt`? One can imagine stake rewards accruing from node goverened token vaults--
funded by DEX fees--periodically dropping tokens into the stake pool.

In reality anyone, i.e., any on chain actor can add value.
Value can come from a revenue generating program, a donation from a community member, or via a
governance controlled vault. It's up to the transaction sender whether they want to just
deposit directly into the staking pool or buy and burn the `spt` in the open market.

However, due to the more stringent requirements of staking on Serum, it's important
to restrict the staking pool token so that it isn't fungible to anyone outside of the
pool. This means redemptions--staking pool withdrawals--require not only the staking pool
token, but an additional level of authorization: proof of stake provided by the Registry.
As a result, one can't buy staking pool tokens on the open market and then redeem them.
Tokens must be staked directly.

### Capabilities

The other way for a Serum Node to earn rewards is by fulfilling various node duties and
earning fees for performing those duties. To start, the first required node duty will be
Cranking. If the idea of cranking is new to you, see
[A technical introduction to the serum Dex](https://docs.google.com/document/d/1isGJES4jzQutI0GtQGuqtrBUqeHxl_xJNXdtOv4SdII/edit#).

As far as staking is concerned, capability rewards are separate. However,
capabililty rewards can use the Registry to determine the fee rate it should pay
out.

For example, when a crank transaction is executed, instead of going directly to the
Serum DEX, it can go to a program with a rewards vault. The program will relay the transaction
to the DEX, and if successful, lookup the fee rate it should pay by looking at Registry
accounts, where it can check if a node `Entity` is eligible for reward, and then payout the
fee to the crank turner.

The trouble here is getting the rewards vault funded. Ideally, DEX
users would fund such a vault, leaving behind transaction fees when they issue orders
that node-leaders collect when they turn cranks. In practice, to avoid breaking changes
to the DEX, such a vault can be funded by a governance mechanism similar to dropping
funds into pools.

### Reward Eligibility

A 1 `MSRM` balance is required to be eligible for rewards. if this balance drops to 0, then
rewards cease to be distributed.

For capabilities, this is straightforward. The capabilities-rewards program can choose to not
pay out the fee by looking at Registry accounts.

For staking, this can be implemented in two parts: 1) creating a withdrawal
timelock for `MSRM` that's longer  than the timelock for `SRM`, and 2) checking for the presence
of `MSRM` when `SRM` is withdrawn. If the `MSRM` balance is zero, then the `SRM` withdrawn
from the staking pool must be less than or equal to the originally staked balance. Importantly,
this means stakers need to monitor the `MSRM` balance of their node. If they see `MSRM`
drop below 1, then they should withdraw immediately to collect their rewards.

## Staking as a Liquidity Pool

Note this design uses the word staking, but it's fundamentally different from staking in the
PoS consensus sense.

Although it does have the property that the more you stake, the more
rewards you get (since you get a higher proportion of the backing pool tokens), it doesn't have
the property that increasing stake to a node increases the node's marginal reward over a given time.
In Tendermint/Cosmos, for example, the more you stake, the more your potential loss (via slashing),
the more transactions you are elected to validate, and so the more transaction fees you earn per epoch.

Stake, in the PoS sense of the word,
is used to measure incentive compatability. That is, the expected value of taking one action--the protocol's
action--should be  more than the expected value of attacking it. There's no analogy that really makes sense here.
This is because Serum nodes aren't tied to security--at least not for this version--but instead are tied to UX,
operational efficiency, and community incentives.

# Interface
[interface]: #interface

## Caveat

The one piece missing from the interface is the pool primitive. The assumption
made is that pools can be configured such that adding assets and redeeming assets
must be signed off by a gatekeeping key, which is the Registry. This is needed,
for example, to enforce withdrawal timelocks. If this assumption is satisfied,
it should be straight forward to add the necessary pool program_id and accounts
to the interfaces here.

## Accounts

The program owns four accounts: `Registrar`, `Entity`, `Member`, and `PendingWithdrawal`.

### Regsitrar

The `Registrar` is a global account defining an instance of the Registry and its configuration.

```rust
pub struct Registrar {
    /// Set by the program on initialization.
    pub initialized: bool,
    /// Priviledged account with the ability to register capabilities.
    pub authority: Pubkey,
    /// Maps capability_id -> bps fee rate earned for performing the capability.
    /// Other programs paying rewards for fulfilling node capabilities can use
    /// this to decide how much they should pay.
    pub capabilities_fees_bps: [u32; 32],
    /// Number of slots one must wait when withdrawing stake.
    pub withdrawal_timelock: u64,
}
```

Most notably, it defines the set of `capabilities_fees_bps` that other programs use
to determine when rewarding capability fulfillment. These fees can be changed by the
`Registrar`'s `authority`, which can perform a priviledged set of governance related
instructions. This `authority` can be a dictatorship or a democratically governed
program-derived-address.

### Entity

An `Entity` account represents a single node collective, i.e., the entity you stake with.
The total SRM equivalent staked cannot exceed 100m.

```rust
pub struct Entity {
    /// Set when this entity is registered with the program.
    pub initialized: bool,
    /// Leader of the entity, i.e., the one responsible for fulfilling node
    /// duties.
    pub leader: Pubkey,
    /// Amount of the token staked to this entity.
    pub amount: u64,
    /// Amount of the mega token staked to this entity.
    pub mega_amount: u64,
    /// Bitmap set representing this entity's capabilities.
    pub capabilities: u32,
    /// Type of stake backing this entity, determining the voting rights
    /// of the stakers. `voting-staked` or `delegated-staked`. Governance
    /// programs can use this information to determine how an `Entity` collective
    /// should be governed.
    pub stake_kind: StakeKind,
    /// The amount of SRM waiting to be staked *before* 1 MSRM has been deposited.
    /// These funds are stored in a program controlled vault, not staked, so this
    /// field is used solely as a signaling mechanism, e.g. to allow Node formation
    /// to gain momentum  before 1MSRM has been deposited, signaling to 1MSRM stakers
    /// that they should stake with this Entity. Once 1MSRM has been staked with this
    //// Entity, these funds can be staked as the associated Members initiate additional
    /// transactions.
    pub stake_intent: u64,
}
```

Note the last field `stake_intent`. This field is an optional optimization to allow
`Member`s to signal their *intent* to stake as soon as 1 MSRM is staked to the Entity.
That is, one can send funds to the Registry to hold before 1 MSRM has been staked, but
they will sit in a vault and not generate rewards. These funds can be withdrawn at any time.
Once a `Member` stakes 1 `MSRM` to the `Entity`, any members with
`stake_intent` balances can tell the `Registry` program to transfer the funds to
the staking pool.

### Member

A `Member` represents a single account staked to an `Entity`.

```rust
pub struct Member {
    /// Set by the program on creation.
    pub initialized: bool,
    /// Entity account providing membership.
    pub entity: Pubkey,
    /// The owner of this account. This key is required to add further stake
    /// or redeem assets on this Member account.
    pub beneficiary: Pubkey,
    /// Amount of SRM staked.
    pub amount: u64,
    /// Amount of MSRM staked.
    pub mega_amount: u64,
    /// Same as the `stake_intent` field in `Enity`, but for the individual
    /// `Member`. The stake_intent field in Entity equals the sum of the
    /// stake_intent field in all the Entity's members.
    pub stake_intent: u64,
    /// Deleate key authorized to deposit or withdraw from the staking pool
    /// on behalf of the beneficiary.
    pub delegate: Pubkey,
    /// The amount deposited into the program on behalf of the delegate.
    pub delegate_amount: u64,
    /// The amount of withdrawals currently in a timelock.
    pub pending_withdrawals: u64,
}
```

It tracks the amount of stake locked in the staking pool so that other programs can
measure proportional stake, e.g., for a governance program to implement stake-based voting.

Note the last three fields.

`delegate` and `delegate_amount` are used to implement staking
for **locked** SRM. The `delegate` field allows another address, namely a program-derived-address controlled by the
SRM lockup program, to both add and withdraw stake on behalf of a user, updating the
`delegate_amount` proportionally.

Furthermore, the `delegate_amount` field is used to safely implement buy and burns on the
staking pool. When selling one's staking pool token to a buy and burn mechanism, one should
provide the associated `Member` account along with a signature from it's beneficiary.
The buy and burn mechanism is then responsible for checking the amount held by the `Member`
account is sufficient (i.e. `total_staked - delegate_amount - buy_and_burn_amount > 0`).
More on this topic in the withdrawals section.

The last field, `pending_withdrawals` is used in conjunction with a timelock on withdrawals.
Governance programs can use this field to know a lower bound on the amount of stake the user will
have over the timelock period, for use in stake-based voting (you don't want someone to be
able to vote and then immediately sell all their stake during the voting period). Alternatively,
a governance program could simply not allow voting if `pending_withdrawals > 0`.

## Registry Initialization and Governance Instructions

### Initialization Instruction

After deploying the `Registry` program on chain, the `Registrar` must be
initialized with an initial configuration.

```rust
/// Accounts:
///
/// 0. `[writable]` Registrar to initialize.
/// 1. `[]`         Rent sysvar.
Initialize {
    authority: Pubkey,
    withdrawal_timelock: u64,
}
```

### RegisteringCapability Instruction

To set fees for a capability, the `Registrar`'s `authority` must register a
capability fee.

```rust
/// RegisterCapability registers a node capability for reward collection,
/// or overwrites an existing capability (e.g., on fee change).
///
/// Accounts:
///
/// 0. `[signer]`   Registrar authority.
/// 1. `[writable]` Registrar instance.
RegisterCapability {
    /// The identifier to assign this capability.
    capability_id: u8,
    /// Capability fee in bps. The amount to pay a node for an instruction fulfilling
    /// this duty.
    capability_fee_bps: u64,
}
```

## Bootstrapping a Node Collective

### Creating a Node Entity

To startup a node, a node-leader must create a Node `Entity` with the Serum `Registry` via the
`CreateEntity` instruction.

```rust
/// Accounts:
///
/// 0. `[writable]` Entity account.
/// 1. `[signer]`   Leader of the node.
/// 2. `[]`         Rent sysvar.
CreateEntity {
  /// The bitset of all capabilities this node can perfrom.
  capabilities: u32,
  /// Type of governance backing the `Entity`. For simplicity in the first version,
  /// all `nodes` will be `delegated-staked`, which means the `node-leader`
  /// will execute governance decisions.
  stake_kind: StakeKind,
}
```

An `Entity` will not be able to stake or fulfill node capabilities until **activated** by
1 MSRM via the `Stake` instruction.

### Obtaining Entity Membership

Joining a node happens via the `JoinEntity` instruction, which initializes a `Member`
account on behalf of a `beneficiary` and an optional `delegate`.

```rust
/// Accounts:
///
/// 0. `[writable]` Member account being created.
/// 1. `[]`         Entity account to stake to.
/// 3. `[]`         Rent sysvar.
JoinEntity {
    /// The owner of this entity account. Must sign off when staking and
    /// withdrawing.
    beneficiary: Pubkey,
    /// An account that can withdrawal or stake on the beneficiary's
    /// behalf.
    delegate: Pubkey,
}
```

As described in the `Member` accounts section, the  `delegate` field is used to
implement staking for locked SRM.

### Staking with an Entity

Staking a node entity happens via the `Stake` instruction, which wil do one of two things.

If an `Entity` is active, i.e., either the stake is >= 1 `MSRM` *or* it already has
1 `MSRM` staked with it, then the `Stake`  instruction will update account balances
and add the SRM  to the staking pool, minting a redeemable staking pool token to the
sender in exchange.

If an `Entity` is not active, the SRM will be marked in accounts as `stake_intent`
and deposited into a program-controlled vault, as described in the the accounts section.

```rust
/// Accounts:
///
/// 0. `[signer]`   Owner of the depositing token account.
/// 1. `[]`         The depositing token account.
/// 2. `[writable]` Member account responsibile for the stake.
/// 3. `[signer]`   Beneficiary *or* delegate of the Member account
///                 being staked.
/// 4. `[writable]` Entity account to stake to.
/// 5. `[]`         SPL token program.
Stake {
    // Amount of of the token to stake with the entity.
    amount: u64,
}
```

### Staking Locked Tokens

Staking locked tokens can be implemented with the above. If the `delegate`
field is set on the `Member` account to a program-derived-address  controlled
by the locked SRM program, then that program can stake and withdrawl on behalf
of a `Member` beneficiary. The locked SRM <-> stake pool transfer is a closed loop,
and the beneficiary can never redeem the tokens inside the staking pool, because
it doesn't control the staking pool tokens created upon deposit--the lockup
program does.

However, although the beneficiary cannot directly take locked tokens out of the
pool, it can tell the locked token program to stake or withdraw stake, updating
the balance of the beneficiary's vesting account. And so if there is a linear
lockup schedule, the vested amount will be automatically adjusted to reflect
the new value from the staking pool redemption.

## Withdrawals

Withdrawals happen over a 1 week time period. As a result there are two transactions
and two instructions that must be executed.

### Starting a Withdrawal

`StartStakeWithdrawal` initiates a withdrawl, incrementing the `pending_withdrawal`
amount for the `Member` and "printing" a `PendingWithdrawl` account as a receipt
to be used at the end of the timelock period to complete the withdrawal.

Importantly, only the beneficiary of a `Member` account can initiate a withdrawal,
and so it signs off on the instruction along with the **owner** of the staking pool
token.

```rust
/// Accounts:
///
/// 0. `[writable]  PendingWithdrawal account to initialize.
/// 0  `[signed]`   Benficiary/delegate of the Stake account.
/// 1. `[writable]` The Member account to withdraw from.
/// 2. `[writable]` Entity the Stake is associated with.
/// 3. `[signed]`   Owner of the staking pool token account to redeem.
StartStakeWithdrawal {
    amount: u64,
    mega_amount: u64,
}
```

### Ending a Withdrawal

Once a withdrawal timelock passes, the `PendingWithdrawal` account can
be provided to the `EndStakeWithdrawal` instruction to complete the redemption.
The `Registry` will redeem the staking pool token for the underlying asset (SRM
or MSRM) and the `PendingWithdrawal` account will be burned so that it cannot
be double spent.

```rust
/// Accounts:
///
/// 0. `[writable]  PendingWithdrawal account to complete.
/// 1. `[signed]`   Beneficiary/delegate of the member account.
/// 2. `[writable]` Member account to withdraw from.
/// 3. `[writable]` Entity account the member is associated with.
/// 4. `[]`         SPL token program (SRM).
/// 5. `[]`         SPL mega token program (MSRM).
/// 6. `[writable]` SRM token account to send to upon redemption
/// 7. `[writable]` MSRM token account to send to upon redemption
EndStakeWithdrawal
```

## Instructions

For completeness, the full instruction interface follows.

```rust
pub enum RegistryInstruction {
    /// Initializes the registry instance for use.
    ///
    /// Accounts:
    ///
    /// 0. `[writable]` Registrar to initialize.
    /// 1. `[]`         Rent sysvar.
    Initialize {
        /// The priviledged account.
        authority: Pubkey,
        /// Number of slots that must pass for a withdrawal to complete.
        withdrawal_timelock: u64,
    },
    /// RegisterCapability registers a node capability for reward collection,
    /// or overwrites an existing capability (e.g., on fee change).
    ///
    /// Accounts:
    ///
    /// 0. `[signer]`   Registrar authority.
    /// 1. `[writable]` Registrar instance.
    RegisterCapability {
        /// The identifier to assign this capability.
        capability_id: u8,
        /// Capability fee in bps. The amount to pay a node for an instruction fulfilling
        /// this duty.
        capability_fee_bps: u32,
    },
    /// CreateEntity initializes the new "node" with the Registry, designated "inactive".
    ///
    /// Accounts:
    ///
    /// 0. `[writable]` Entity account.
    /// 1. `[signer]`   Leader of the node.
    /// 2. `[]`         Rent sysvar.
    CreateEntity {
        /// The Serum ecosystem duties a Node performs to earn extra performance
        /// based rewards, for example, cranking.
        capabilities: u32,
        /// Type of governance backing the `Entity`. For simplicity in the first version,
        /// all `nodes` will be `delegated-staked`, which means the `node-leader`
        /// will execute governance decisions.
        stake_kind: crate::accounts::StakeKind,
    },
    /// Joins the entity by creating a membership account.
    ///
    /// Accounts:
    ///
    /// 0. `[writable]` Member account being created.
    /// 1. `[]`         Entity account to stake to.
    /// 3. `[]`         Rent sysvar.
    JoinEntity {
        /// The owner of this entity account. Must sign off when staking and
        /// withdrawing.
        beneficiary: Pubkey,
        /// An account that can withdrawal or stake on the beneficiary's
        /// behalf.
        delegate: Pubkey,
    },
    /// Deposits funds into the staking pool on behalf Member account of
    /// the Member account, issuing staking pool tokens as proof of deposit.
    ///
    /// If 1 MSRM is not present either in the `amount` field or the `Entity`
    /// then locks funds in a `stake_intent` vault.
    ///
    /// Accounts:
    ///
    /// 0. `[signer]`   Owner of the depositing token account.
    /// 1. `[]`         The depositing token account.
    /// 2. `[writable]` Member account responsibile for the stake.
    /// 3. `[signer]`   Beneficiary *or* delegate of the Member account
    ///                 being staked.
    /// 4. `[writable]` Entity account to stake to.
    /// 5. `[]`         SPL token program.
    Stake {
        // Amount of of the token to stake with the entity.
        amount: u64,
        // True iff staking MSRM.
        is_mega: bool,
    },
    /// Initiates a stake withdrawal. Funds are locked up until the
    /// withdrawl timelock passes.
    ///
    /// Accounts:
    ///
    /// 0. `[writable]  PendingWithdrawal account to initialize.
    /// 0  `[signed]`   Benficiary/delegate of the Stake account.
    /// 1. `[writable]` The Member account to withdraw from.
    /// 2. `[writable]` Entity the Stake is associated with.
    /// 3. `[signed]`   Owner of the staking pool token account to redeem.
    StartStakeWithdrawal { amount: u64, mega_amount: u64 },
    /// Completes the pending withdrawal once the timelock period passes.
    ///
    /// Accounts:
    ///
    /// Accounts:
    ///
    /// 0. `[writable]  PendingWithdrawal account to complete.
    /// 1. `[signed]`   Beneficiary/delegate of the member account.
    /// 2. `[writable]` Member account to withdraw from.
    /// 3. `[writable]` Entity account the member is associated with.
    /// 4. `[]`         SPL token program (SRM).
    /// 5. `[]`         SPL mega token program (MSRM).
    /// 6. `[writable]` SRM token account to send to upon redemption
    /// 7. `[writable]` MSRM token account to send to upon redemption
    EndStakeWithdrawal,
}
```
