use serum_common::pack::Pack;
use serum_lockup::instruction::LockupInstruction;
use serum_registry::access_control;
use serum_registry::accounts::{EntityState, LockedRewardVendor, Member, Registrar};
use serum_registry::error::{RegistryError, RegistryErrorCode};
use solana_program::info;
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::sysvar;

#[inline(never)]
pub fn handler(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    cursor: u32,
    nonce: u8,
) -> Result<(), RegistryError> {
    info!("handler: claim_locked_reward");

    let acc_infos = &mut accounts.iter();

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

    let AccessControlResponse {
        ref registrar,
        ref vendor,
    } = access_control(AccessControlRequest {
        program_id,
        registrar_acc_info,
        entity_acc_info,
        member_acc_info,
        vendor_acc_info,
        cursor,
    })?;

    Member::unpack_mut(
        &mut member_acc_info.try_borrow_mut_data()?,
        &mut |member: &mut Member| {
            state_transition(StateTransitionRequest {
                cursor,
                nonce,
                member,
                vendor,
                registrar,
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
            })
            .map_err(Into::into)
        },
    )?;

    Ok(())
}

fn access_control(req: AccessControlRequest) -> Result<AccessControlResponse, RegistryError> {
    info!("access-control: claim_locked_reward");

    let AccessControlRequest {
        program_id,
        cursor,
        registrar_acc_info,
        entity_acc_info,
        member_acc_info,
        vendor_acc_info,
    } = req;

    // Authorization: none required. This operation is purely beneficial for
    //                the member account.

    // Account validation.
    let registrar = access_control::registrar(registrar_acc_info, program_id)?;
    let entity = access_control::entity(entity_acc_info, registrar_acc_info, program_id)?;
    let member = access_control::member_belongs_to(
        member_acc_info,
        registrar_acc_info,
        entity_acc_info,
        program_id,
    )?;
    let vendor =
        access_control::locked_reward_vendor(vendor_acc_info, registrar_acc_info, program_id)?;

    // ClaimLockedReward specific.
    //
    // Is the cursor valid?
    if vendor.reward_event_q_cursor != cursor {
        return Err(RegistryErrorCode::InvalidCursor)?;
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

    Ok(AccessControlResponse { registrar, vendor })
}

fn state_transition(req: StateTransitionRequest) -> Result<(), RegistryError> {
    info!("state-transition: claim_locked_reward");

    let StateTransitionRequest {
        cursor,
        member,
        vendor,
        safe_acc_info,
        lockup_program_acc_info,
        vendor_vault_authority_acc_info,
        registrar,
        registrar_acc_info,
        vendor_acc_info,
        vesting_acc_info,
        vesting_vault_acc_info,
        vendor_vault_acc_info,
        token_program_acc_info,
        rent_acc_info,
        clock_acc_info,
        nonce,
    } = req;

    // Create vesting account with proportion of the reward.
    let spt = {
        if vendor.pool == registrar.pool_vault {
            member.balances.spt_amount
        } else {
            member.balances.spt_mega_amount
        }
    };
    let amount = spt
        .checked_div(vendor.pool_token_supply)
        .unwrap()
        .checked_mul(vendor.total)
        .unwrap();
    let ix = {
        let instr = LockupInstruction::CreateVesting {
            beneficiary: member.beneficiary,
            end_ts: vendor.end_ts,
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

    // Move member rewards cursor.
    member.rewards_cursor = cursor + 1;

    Ok(())
}

struct AccessControlRequest<'a, 'b> {
    program_id: &'a Pubkey,
    cursor: u32,
    entity_acc_info: &'a AccountInfo<'b>,
    member_acc_info: &'a AccountInfo<'b>,
    registrar_acc_info: &'a AccountInfo<'b>,
    vendor_acc_info: &'a AccountInfo<'b>,
}

struct AccessControlResponse {
    registrar: Registrar,
    vendor: LockedRewardVendor,
}

struct StateTransitionRequest<'a, 'b, 'c> {
    cursor: u32,
    nonce: u8,
    member: &'c mut Member,
    vendor: &'c LockedRewardVendor,
    registrar: &'c Registrar,
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
}
