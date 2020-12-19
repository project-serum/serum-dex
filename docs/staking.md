# Serum Staking

WARNING: All code related to Serum Staking is unaudited. Use at your own risk.

## Introduction

The **Registry** program provides the central point of on-chain coordination for stakers,
providing two features: a gateway to the staking pool and a repository for node state.

As a gateway to the staking pool, the **Registry** controls who and when stakers can enter
and exit the pool. This enables controls like a mandatory 1 MSRM node deposit before entering
the pool, the staking of *locked* SRM, and a 1 week timelock for withdrawals. As a repository for 
node state, the **Registry** allows other programs to
perform access control on staked accounts. This is useful for programs building on-top of the
registry. For example, a **crank-rewards** program could use Registry accounts to determine if
a node-leader is eligble for payment. A **governance** program could tally votes from
node member accounts based on their stake weight.

In short, the **Registry** allows node entities to be created, members to stake, and rewards to
be generated, while providing a foundation for other programs to build on.

Here, we'll discuss it's role in facilitating staking.

## Creating a member account.

Before being able to enter the stake pool, one must create a [Member](https://github.com/project-serum/serum-dex/blob/master/registry/src/accounts/member.rs) account with the
[Registrar](https://github.com/project-serum/serum-dex/blob/master/registry/src/accounts/registrar.rs), providing identity to the **Registry** program. By default, each member has four types of token vaults making up a set
of balances owned by the program on behalf of a **Member**:

* Free-balances
* Pending
* Stake
* Stake pool token

Each of these vaults provide a unit of balance isolation unique to a **Member**.
That is, although we provide a pooling mechanism, funds between **Member** accounts do not
share SPL token accounts. The only way for funds to move is for a **Member** to authorize
instructions that either exit the system or move funds between a **Member**'s own vaults.

## Depositing and Withdrawing.

Funds enter and exit the **Registry** through the `Deposit` and `Withdraw` instructions,
which transfer funds into and out of the **free-balances** vault.
As the name suggests, all funds in this vault are freely available, unrestricted, and
earn zero interest. The vault is purely a gateway for funds to enter the system. One
could even perform token transfer directly into the vault, but it's recommended to use the
instruction API, which provide additional safety checks to help ensure the funds are moved
as intended.

## Staking.

Once deposited, **Members** invoke the `Stake` instruction to transfer funds from
their **free-balances-vault** to their **stake-vault**, creating newly minted
**stake-pool-tokens** as proof of the stake deposit. These new tokens represent
one's proportional right to all rewards distributed to the staking pool and are offered
by the **Registry** program at a fixed price of 1000 SRM--to start and subject to change
in future versions. This creates some restrictions on the underlying stake.

## Unstaking

Once staked, funds cannot be immediately withdrawn. Rather, the **Registry** will enforce
a one week timelock before funds are released. Upon executing the `StartStakeWithdrawal`
instruction, three operations execute. 1) The given amount of stake pool tokens will be burned.
2) Staked funds proportional to the stake pool tokens burned will be transferred from the
**Member**'s **stake-vault** to the **Member**'s **pending-vault**. 3) A `PendingWithdrawal`
account will be created as proof of the stake withdrawal, stamping the current block's
`unix_timestamp` onto the account. When the timelock period ends, a **Member** can invoke the
`EndStakeWithdrawal` instruction to complete the transfer out of the `pending-vault` and
into the `free-balances`, providing the previously printed `PendingWithdrawal`
receipt to the program as proof that the timelock has passed. At this point, the exit
from the stake pool is complete, and the funds are ready to be used again.

## Reward Design Motivation

Feel free to skip this section and jump to the **Reward Vendors** section if you want to
just see how rewards work.

One could imagine several ways to drop rewards onto a staking pool, each with their own downsides.
Of course what you want is, for a given reward amount, to atomically snapshot the state
of the staking pool and to distribute it proportionally to all stake holders. Effectively,
an on chain program such as

```python
for account in stake_pool:
  account.token_amount += total_reward * (account.stake_pool_token.amount / stake_pool_token.supply)
 ```

Surprisingly, such a mechanism is not immediately obvious.

First, the above program is a non starter. Not only does the SPL token
program not have the ability to iterate through all accounts for a given mint within a program,
but, since Solana transactions require the specification of all accounts being accessed
in a transaction (this is how it achieves parallelism), such a transaction's size would be
well over the limit. So modifying global state atomically in a single transaction is out of the
question.

So if you can't do this on chain, one can try doing it off chain. One could write an program to
snapshot the pool state, and just airdrop tokens onto the pool. This works, but
adds an additional layer of trust. Who snapshots the pool state? At what time?
How do you know they calculated the rewards correctly? What happens if my reward was not given?
This is not auditable or verifiable. And if you want to answer these questions, requires
complex off-chain protocols that require either fancy cryptography or effectively
recreating a BFT system off chain.

Another solution considerered was to use a uniswap-style AMM pool (without the swapping).
This has a lot of advantages. First it's easy to reason about and implement in a single transaction.
To drop rewards gloablly onto the pool, one can deposit funds directly into the pool, in which case
the reward is automatically received by owners of the staking pool token upon redemption, a process
known as "gulping"--since dropping rewards increases the total value of the pool
while their proportion of the pool remained constant.

However, there are enough downsides with using an AMM style pool to offset the convience.
Unfortunately, we lose the nice balance isolation property **Member** accounts have, because
tokens have to be pooled into the same vault, which is an additional security concern that could
easily lead to loss of funds, e.g., if there's a bug in the redemption calculation. Moreover, dropping
arbitrary tokens onto the pool is a challenge. Not only do you have to create new pool vaults for
every new token you drop onto the pool, but you also need to have stakers purchase those tokens to enter
the pool. So not only are we staking SRM, but we're also staking other tokens. An additional oddity is that
as rewards are dropped onto the pool, the price to enter the pool monotonically increases. Remember, entering this
type of pool requires "creating" pool tokens, i.e., depositing enough tokens so that you don't dilute
any other member. So if a single pool token represents one SRM. And if a single SRM is dropped onto every
member of the pool, all the existing member's shares are now worth two SRM. So to enter the pool without
dilution, one would have to "create" at a price of 2 SRM per share. This means that rewarding
stakers becomes more expensive over time. One could of course solve this problem by implementing
arbitrary `n:m` pool token splits, which leads us right back to the problem of mutating global account
state for an SPL token. Furthermore, we haven't even touched upon dropping locked token rewards,
which of course can't be dropped directly onto a pool, since they are controlled by an additional
program controlling it's own set of accounts. So, if we did go with an AMM style pool, we'd need a separate
mechanism for handling locked token rewards. Ideally, we'd have a single mechanism for both.

## Reward Vendors

Instead of trying to *push* rewards to users via a direct transfer or airdrop, we can use a *polling* model
where users effectively event source a log on demand.

When a reward is created, we do two things:

1) Create a **Reward Vendor** account with an associated token vault holding the reward.
2) Assign the **Reward Vendor** the next available position in a **Reward Event Queue**. Then, to retrieve
a reward, a staker invokes the `ClaimReward` command, providing a proof that the funds were
staked at the time of the reward being dropped, and in response, the program transfers or,
some might say, *vends* the proportion of the dropped reward to the polling **Member**. The
operation completes by incrementing the **Member**'s queue cursor, ensuring that a given
reward can only be processed once.

This allows us to provide a way of dropping rewards to the stake pool in a way that is
on chain and verifiable. Of course, it requires an external trigger, some account willing to
transfer funds to a new **RewardVendor**, but that is outside of the scope of the **Registry**
program. The reward dropper can be an off chain BFT committee, or it can be an on-chain multisig. It can be a charitable individual,
or funds can flow directly from the DEX, which itself creates a Reward Vendor from fees collected. 
It doesn't matter to the **Registry** program.

Note that this solution also allows for rewards to be denominated in any token, not just SRM.
Since rewards are paid out by the vendor immediately and to a token account of the **Member**'s
choosing, it *just works*. Even more, this extends to arbitrary program accounts, particularly
**Locked SRM**. A **Reward Vendor** needs to additionally know the accounts and instruction data
to relay to the program, but otherwise, the mechanism is the same. The details of **Locked SRM** will
be explained in an additional document.


## Reward Eligibility

To be eligible for reward, a node **Entity** must have 1 MSRM staked to it, marking
it "active". If the MSRM stake balance ever drops below 1, the node will be marked as pending
deactivation, starting a week long countdown ending with the **Entity** transitioning into the
inactive state, at which point rewards cease to be distributed. As soon one enters the pending
deactivation state, 1 MSRM needs to either be restaked by an associated **Member**
or all **Members** should move to a new **Entity**. Note that transitioning to an inactive state
does not affect one's **Member** vaults. It only affects one's ability to retrieve rewards from
a vendor.

## Misc

### Entity

An **Entity** is an additional **Registry** owned account representing a collection of **Member**
accounts with an associated "node leader", who is eligible for additional rewards via "node duties".
These "duties" amount to earning what are, effectively, transaction fees. An additional document will describe nodes
and their setup, but for the purposes of staking, all a **Member** needs to know is that it
belongs to an **Entity** account, and the stake associated with that **Entity** determines
its state.

### Member Accounts

This document describes 4 vault types belonging to **Member** accounts, making up a single set of balances. However,
there are two stake pools: one for SRM holders and one for MSRM holders, so really there are 8 vault types.
Additionally there are two types of balance groups: locked and unlocked. 
As a result, there are really 16 vaults for each **Member**, 8 types of vaults in 2 separate sets, 
each isolated from the other, so that locked tokens don't get intermingled with unlocked tokens. 

But if we're staking locked tokens, we need to ensure we don't accidently unlock tokens. 
To maintain the **Lockup** program's invariant, we need a mechanism for safely entering and exiting 
the system; that is, locked tokens should only be sent back to the lockup program. 

As a result, we assign each set of balances, locked and unlocked, it's own unique identifier. 
For the unlocked set of accounts the  identifier is the **Member** account's beneficiary 
(i.e. the authority of the entire account), and for the locked set of accounts it's the vesting account's program
derived address, controlled by the lockup program. Upon depositing or withdrawing from the **Registry**,
the program ensures that tokens coming into the system are from vaults owned by the correct balance 
identifier. Similarly, tokens going out of the system can only go to vaults owned by the correct balance
identifier.

In future work, this setup will allow us to extend the staking program to stake arbitrary assets owned by 
arbitrary programs on behalf of an account owner.
