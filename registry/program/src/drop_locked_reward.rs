use serum_common::pack::Pack;
use serum_common::program::invoke_token_transfer;
use serum_registry::access_control;
use serum_registry::accounts::reward_queue::Ring;
use serum_registry::accounts::{LockedRewardVendor, RewardEvent, RewardEventQueue};
use serum_registry::error::{RegistryError, RegistryErrorCode};
use solana_program::msg;
use solana_sdk::account_info::next_account_info;
use solana_sdk::account_info::AccountInfo;
use solana_sdk::program_pack::Pack as TokenPack;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::sysvar::clock::Clock;
use spl_token::state::{Account as TokenAccount, Mint};
use std::convert::Into;

#[inline(never)]
pub fn handler(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    total: u64,
    end_ts: i64,
    expiry_ts: i64,
    expiry_receiver: Pubkey,
    period_count: u64,
    nonce: u8,
) -> Result<(), RegistryError> {
    msg!("handler: drop_locked_reward");

    let acc_infos = &mut accounts.iter();

    let reward_event_q_acc_info = next_account_info(acc_infos)?;
    let registrar_acc_info = next_account_info(acc_infos)?;
    let depositor_acc_info = next_account_info(acc_infos)?;
    let depositor_owner_acc_info = next_account_info(acc_infos)?;
    let pool_mint_acc_info = next_account_info(acc_infos)?;
    let vendor_acc_info = next_account_info(acc_infos)?;
    let vendor_vault_acc_info = next_account_info(acc_infos)?;
    let token_program_acc_info = next_account_info(acc_infos)?;
    let clock_acc_info = next_account_info(acc_infos)?;

    let AccessControlResponse {
        pool_mint,
        ref clock,
    } = access_control(AccessControlRequest {
        program_id,
        reward_event_q_acc_info,
        registrar_acc_info,
        pool_mint_acc_info,
        vendor_acc_info,
        vendor_vault_acc_info,
        clock_acc_info,
        expiry_ts,
        end_ts,
        nonce,
        total,
    })?;

    LockedRewardVendor::unpack_mut(
        &mut vendor_acc_info.try_borrow_mut_data()?,
        &mut |vendor: &mut LockedRewardVendor| {
            state_transition(StateTransitionRequest {
                nonce,
                total,
                end_ts,
                expiry_ts,
                expiry_receiver,
                vendor,
                vendor_acc_info,
                vendor_vault_acc_info,
                reward_event_q_acc_info,
                registrar_acc_info,
                depositor_acc_info,
                depositor_owner_acc_info,
                token_program_acc_info,
                pool_mint,
                pool_mint_acc_info,
                clock,
                period_count,
            })
            .map_err(Into::into)
        },
    )?;

    Ok(())
}

fn access_control(req: AccessControlRequest) -> Result<AccessControlResponse, RegistryError> {
    msg!("access-control: drop_locked_reward");

    let AccessControlRequest {
        program_id,
        reward_event_q_acc_info,
        registrar_acc_info,
        pool_mint_acc_info,
        vendor_acc_info,
        vendor_vault_acc_info,
        clock_acc_info,
        expiry_ts,
        end_ts,
        nonce,
        total,
    } = req;

    // Authorization: none.

    // Account validation.
    let clock = access_control::clock(clock_acc_info)?;
    let registrar = access_control::registrar(registrar_acc_info, program_id)?;
    let _reward_q = access_control::reward_event_q(
        reward_event_q_acc_info,
        registrar_acc_info,
        &registrar,
        program_id,
    )?;
    let pool_mint = access_control::mint(pool_mint_acc_info)?;
    if registrar.pool_mint != *pool_mint_acc_info.key
        && registrar.pool_mint_mega != *pool_mint_acc_info.key
    {
        return Err(RegistryErrorCode::InvalidPoolTokenMint)?;
    }
    let vendor_vault_authority = Pubkey::create_program_address(
        &[
            registrar_acc_info.key.as_ref(),
            vendor_acc_info.key.as_ref(),
            &[nonce],
        ],
        program_id,
    )
    .map_err(|_| RegistryErrorCode::InvalidVaultAuthority)?;
    let _vault = access_control::token(vendor_vault_acc_info, &vendor_vault_authority)?;

    if LockedRewardVendor::initialized(vendor_acc_info)? {
        return Err(RegistryErrorCode::AlreadyInitialized)?;
    }
    if clock.unix_timestamp >= end_ts {
        return Err(RegistryErrorCode::InvalidEndTs)?;
    }
    if clock.unix_timestamp >= expiry_ts {
        return Err(RegistryErrorCode::InvalidExpiry)?;
    }
    // Must be dropping enough to give at least one token to everyone in pool.
    if total < pool_mint.supply {
        return Err(RegistryErrorCode::InsufficientReward)?;
    }

    Ok(AccessControlResponse { pool_mint, clock })
}

#[inline(always)]
fn state_transition(req: StateTransitionRequest) -> Result<(), RegistryError> {
    msg!("state-transition: drop_locked_reward");

    let StateTransitionRequest {
        nonce,
        total,
        expiry_ts,
        expiry_receiver,
        vendor,
        vendor_acc_info,
        vendor_vault_acc_info,
        reward_event_q_acc_info,
        registrar_acc_info,
        depositor_acc_info,
        depositor_owner_acc_info,
        token_program_acc_info,
        pool_mint,
        pool_mint_acc_info,
        clock,
        period_count,
        end_ts,
    } = req;

    let mint = {
        let t = TokenAccount::unpack(&depositor_acc_info.try_borrow_data()?)?;
        t.mint
    };

    // Emit a reward event.
    let reward_event_q = RewardEventQueue::from(reward_event_q_acc_info.data.clone());
    let cursor = reward_event_q.head_cursor()?;
    reward_event_q.append(&RewardEvent::LockedAlloc {
        from: *depositor_owner_acc_info.key,
        pool: *pool_mint_acc_info.key,
        total,
        vendor: *vendor_acc_info.key,
        mint,
        ts: clock.unix_timestamp,
    })?;

    // Transfer the reward to the vendor.
    invoke_token_transfer(
        depositor_acc_info,
        vendor_vault_acc_info,
        depositor_owner_acc_info,
        token_program_acc_info,
        &[],
        total,
    )?;

    // Initialize the reward vendor account.
    vendor.initialized = true;
    vendor.registrar = *registrar_acc_info.key;
    vendor.vault = *vendor_vault_acc_info.key;
    vendor.nonce = nonce;
    vendor.pool = *pool_mint_acc_info.key;
    vendor.pool_token_supply = pool_mint.supply;
    vendor.reward_event_q_cursor = cursor;
    vendor.start_ts = clock.unix_timestamp;
    vendor.end_ts = end_ts;
    vendor.expiry_ts = expiry_ts;
    vendor.expiry_receiver = expiry_receiver;
    vendor.total = total;
    vendor.period_count = period_count;

    Ok(())
}

struct AccessControlRequest<'a, 'b> {
    program_id: &'a Pubkey,
    reward_event_q_acc_info: &'a AccountInfo<'b>,
    registrar_acc_info: &'a AccountInfo<'b>,
    pool_mint_acc_info: &'a AccountInfo<'b>,
    vendor_acc_info: &'a AccountInfo<'b>,
    vendor_vault_acc_info: &'a AccountInfo<'b>,
    clock_acc_info: &'a AccountInfo<'b>,
    expiry_ts: i64,
    end_ts: i64,
    nonce: u8,
    total: u64,
}

struct AccessControlResponse {
    pool_mint: Mint,
    clock: Clock,
}

struct StateTransitionRequest<'a, 'b, 'c> {
    nonce: u8,
    total: u64,
    end_ts: i64,
    expiry_ts: i64,
    period_count: u64,
    expiry_receiver: Pubkey,
    clock: &'c Clock,
    vendor: &'c mut LockedRewardVendor,
    vendor_acc_info: &'a AccountInfo<'b>,
    vendor_vault_acc_info: &'a AccountInfo<'b>,
    registrar_acc_info: &'a AccountInfo<'b>,
    reward_event_q_acc_info: &'a AccountInfo<'b>,
    pool_mint_acc_info: &'a AccountInfo<'b>,
    depositor_acc_info: &'a AccountInfo<'b>,
    depositor_owner_acc_info: &'a AccountInfo<'b>,
    token_program_acc_info: &'a AccountInfo<'b>,
    pool_mint: Mint,
}
