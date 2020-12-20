use crate::common::entity::{with_entity, EntityContext};
use serum_common::pack::Pack;
use serum_lockup::instruction::LockupInstruction;
use serum_registry::access_control;
use serum_registry::accounts::{Entity, EntityState, LockedRewardVendor, Member, Registrar};
use serum_registry::error::{RegistryError, RegistryErrorCode};
use solana_program::msg;
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::sysvar;
use solana_sdk::sysvar::clock::Clock;
use spl_token::state::Account as TokenAccount;

#[inline(never)]
pub fn handler(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    cursor: u32,
    nonce: u8,
) -> Result<(), RegistryError> {
    msg!("handler: claim_locked_reward");

    let acc_infos = &mut accounts.iter();

    let signer_acc_info = next_account_info(acc_infos)?;
    let entity_acc_info = next_account_info(acc_infos)?;
    let member_acc_info = next_account_info(acc_infos)?;
    let registrar_acc_info = next_account_info(acc_infos)?;
    let vendor_acc_info = next_account_info(acc_infos)?;
    let vendor_vault_acc_info = next_account_info(acc_infos)?;
    let vendor_vault_authority_acc_info = next_account_info(acc_infos)?;
    let safe_acc_info = next_account_info(acc_infos)?;
    let lockup_program_acc_info = next_account_info(acc_infos)?;
    let vesting_acc_info = next_account_info(acc_infos)?;
    let vesting_vault_acc_info = next_account_info(acc_infos)?;
    let token_program_acc_info = next_account_info(acc_infos)?;
    let rent_acc_info = next_account_info(acc_infos)?;
    let clock_acc_info = next_account_info(acc_infos)?;

    let mut spt_acc_infos = vec![];
    // 2: Main and locked stake pool tokens for a given pool.
    for _ in 0..2 {
        spt_acc_infos.push(next_account_info(acc_infos)?);
    }

    let ctx = EntityContext {
        entity_acc_info,
        registrar_acc_info,
        clock_acc_info,
        program_id,
    };
    with_entity(ctx, &mut |entity: &mut Entity,
                           registrar: &Registrar,
                           ref clock: &Clock| {
        let AccessControlResponse {
            ref vendor,
            ref spts,
        } = access_control(AccessControlRequest {
            program_id,
            entity,
            registrar,
            signer_acc_info,
            registrar_acc_info,
            entity_acc_info,
            member_acc_info,
            vendor_acc_info,
            cursor,
            spt_acc_infos: spt_acc_infos.as_ref(),
        })?;

        Member::unpack_mut(
            &mut member_acc_info.try_borrow_mut_data()?,
            &mut |member: &mut Member| {
                state_transition(StateTransitionRequest {
                    cursor,
                    nonce,
                    member,
                    vendor,
                    registrar_acc_info,
                    vendor_acc_info,
                    vendor_vault_authority_acc_info,
                    safe_acc_info,
                    lockup_program_acc_info,
                    vesting_acc_info,
                    vesting_vault_acc_info,
                    vendor_vault_acc_info,
                    token_program_acc_info,
                    rent_acc_info,
                    clock_acc_info,
                    clock,
                    spts,
                })
                .map_err(Into::into)
            },
        )
        .map_err(Into::into)
    })
}

fn access_control(req: AccessControlRequest) -> Result<AccessControlResponse, RegistryError> {
    msg!("access-control: claim_locked_reward");

    let AccessControlRequest {
        program_id,
        cursor,
        registrar,
        entity,
        signer_acc_info,
        registrar_acc_info,
        entity_acc_info,
        member_acc_info,
        vendor_acc_info,
        spt_acc_infos,
    } = req;

    // Authorization. Must be either the beneficiary or the node leader.
    if !signer_acc_info.is_signer {
        return Err(RegistryErrorCode::Unauthorized)?;
    }

    // Account validation.
    let member = access_control::member_registrar_entity(
        member_acc_info,
        registrar_acc_info,
        entity_acc_info,
        program_id,
    )?;
    let vendor =
        access_control::locked_reward_vendor(vendor_acc_info, registrar_acc_info, program_id)?;
    let is_mega = vendor.pool == registrar.pool_mint_mega;
    let spts = spt_acc_infos
        .iter()
        .enumerate()
        .map(|(idx, spt_acc_info)| {
            if is_mega {
                if &member.balances[idx].spt_mega != spt_acc_info.key {
                    return Err(RegistryErrorCode::InvalidSpt)?;
                }
            } else {
                if &member.balances[idx].spt != spt_acc_info.key {
                    return Err(RegistryErrorCode::InvalidSpt)?;
                }
            }
            access_control::token_account(spt_acc_info)
        })
        .collect::<Result<_, _>>()?;

    // Is the cursor valid?
    if vendor.reward_event_q_cursor != cursor {
        return Err(RegistryErrorCode::InvalidCursor)?;
    }
    if vendor.expired {
        return Err(RegistryErrorCode::VendorAlreadyExpired)?;
    }
    // Has the member account processed this cursor already?
    if member.rewards_cursor > cursor {
        return Err(RegistryErrorCode::AlreadyProcessedCursor)?;
    }
    // Was the member staked at the time of reward?
    if member.last_stake_ts > vendor.start_ts {
        return Err(RegistryErrorCode::IneligibleReward)?;
    }
    // Is the entity active?
    if entity.state == EntityState::Inactive {
        return Err(RegistryErrorCode::EntityNotActivated)?;
    }
    // Is the beneficiary or the node leader the signer?
    if &entity.leader != signer_acc_info.key && &member.beneficiary != signer_acc_info.key {
        return Err(RegistryErrorCode::Unauthorized)?;
    }

    Ok(AccessControlResponse { vendor, spts })
}

fn state_transition(req: StateTransitionRequest) -> Result<(), RegistryError> {
    msg!("state-transition: claim_locked_reward");

    let StateTransitionRequest {
        cursor,
        member,
        vendor,
        safe_acc_info,
        lockup_program_acc_info,
        vendor_vault_authority_acc_info,
        registrar_acc_info,
        vendor_acc_info,
        vesting_acc_info,
        vesting_vault_acc_info,
        vendor_vault_acc_info,
        token_program_acc_info,
        rent_acc_info,
        clock_acc_info,
        nonce,
        spts,
        clock,
    } = req;

    // Move rewards cursor.
    member.rewards_cursor = cursor + 1;

    if vendor.expired {
        msg!("Vendor expired. Reward not collected");
        return Ok(());
    }

    // Create vesting account with proportion of the reward.
    let spt_total = spts.iter().map(|a| a.amount).fold(0, |a, b| a + b);
    let amount = spt_total
        .checked_mul(vendor.total)
        .unwrap()
        .checked_div(vendor.pool_token_supply)
        .unwrap();

    if amount <= 0 {
        // Invariant violation.
        msg!("Invalid reward calculation.");
        return Err(RegistryErrorCode::Unknown)?;
    }

    // Lockup program requires the timestamp to be >= clock's timestamp.
    // So update if the time has already passed. 60 seconds is arbitrary.
    let end_ts = {
        if vendor.end_ts <= clock.unix_timestamp + 60 {
            clock.unix_timestamp + 60
        } else {
            vendor.end_ts
        }
    };

    let ix = {
        let instr = LockupInstruction::CreateVesting {
            beneficiary: member.beneficiary,
            end_ts,
            period_count: vendor.period_count,
            deposit_amount: amount,
            nonce,
        };
        let mut data = vec![0u8; instr.size()? as usize];
        LockupInstruction::pack(instr, &mut data)?;
        Instruction {
            program_id: *lockup_program_acc_info.key,
            accounts: vec![
                AccountMeta::new(*vesting_acc_info.key, false),
                AccountMeta::new(vendor.vault, false),
                AccountMeta::new_readonly(*vendor_vault_authority_acc_info.key, true),
                AccountMeta::new(*vesting_vault_acc_info.key, false),
                AccountMeta::new_readonly(*safe_acc_info.key, false),
                AccountMeta::new_readonly(spl_token::ID, false),
                AccountMeta::new_readonly(sysvar::rent::ID, false),
                AccountMeta::new_readonly(sysvar::clock::ID, false),
            ],
            data,
        }
    };
    let signer_seeds = &[
        registrar_acc_info.key.as_ref(),
        vendor_acc_info.key.as_ref(),
        &[vendor.nonce],
    ];
    solana_sdk::program::invoke_signed(
        &ix,
        &[
            vesting_acc_info.clone(),
            vendor_vault_acc_info.clone(),
            vendor_vault_authority_acc_info.clone(),
            vesting_vault_acc_info.clone(),
            safe_acc_info.clone(),
            token_program_acc_info.clone(),
            rent_acc_info.clone(),
            clock_acc_info.clone(),
            lockup_program_acc_info.clone(),
        ],
        &[signer_seeds],
    )?;

    Ok(())
}

struct AccessControlRequest<'a, 'b, 'c> {
    program_id: &'a Pubkey,
    cursor: u32,
    entity: &'c Entity,
    registrar: &'c Registrar,
    signer_acc_info: &'a AccountInfo<'b>,
    entity_acc_info: &'a AccountInfo<'b>,
    member_acc_info: &'a AccountInfo<'b>,
    registrar_acc_info: &'a AccountInfo<'b>,
    vendor_acc_info: &'a AccountInfo<'b>,
    spt_acc_infos: &'c [&'a AccountInfo<'b>],
}

struct AccessControlResponse {
    vendor: LockedRewardVendor,
    spts: Vec<TokenAccount>,
}

struct StateTransitionRequest<'a, 'b, 'c> {
    cursor: u32,
    nonce: u8,
    clock: &'c sysvar::clock::Clock,
    member: &'c mut Member,
    vendor: &'c LockedRewardVendor,
    registrar_acc_info: &'a AccountInfo<'b>,
    vendor_acc_info: &'a AccountInfo<'b>,
    vendor_vault_authority_acc_info: &'a AccountInfo<'b>,
    safe_acc_info: &'a AccountInfo<'b>,
    lockup_program_acc_info: &'a AccountInfo<'b>,
    vesting_acc_info: &'a AccountInfo<'b>,
    vesting_vault_acc_info: &'a AccountInfo<'b>,
    vendor_vault_acc_info: &'a AccountInfo<'b>,
    token_program_acc_info: &'a AccountInfo<'b>,
    rent_acc_info: &'a AccountInfo<'b>,
    clock_acc_info: &'a AccountInfo<'b>,
    spts: &'c [TokenAccount],
}
