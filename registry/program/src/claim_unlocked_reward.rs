use serum_common::pack::Pack;
use serum_common::program::invoke_token_transfer;
use serum_registry::access_control;
use serum_registry::accounts::{EntityState, Member, UnlockedRewardVendor};
use serum_registry::error::{RegistryError, RegistryErrorCode};
use solana_program::msg;
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::pubkey::Pubkey;
use spl_token::state::Account as TokenAccount;

#[inline(never)]
pub fn handler(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    cursor: u32,
) -> Result<(), RegistryError> {
    msg!("handler: claim_unlocked_reward");

    let acc_infos = &mut accounts.iter();

    let signer_acc_info = next_account_info(acc_infos)?;
    let entity_acc_info = next_account_info(acc_infos)?;
    let member_acc_info = next_account_info(acc_infos)?;
    let registrar_acc_info = next_account_info(acc_infos)?;
    let vendor_acc_info = next_account_info(acc_infos)?;
    let vendor_vault_acc_info = next_account_info(acc_infos)?;
    let vendor_vault_authority_acc_info = next_account_info(acc_infos)?;
    let token_acc_info = next_account_info(acc_infos)?;
    let token_program_acc_info = next_account_info(acc_infos)?;

    let mut spt_acc_infos = vec![];
    while acc_infos.len() > 0 {
        spt_acc_infos.push(next_account_info(acc_infos)?);
    }

    let AccessControlResponse {
        ref vendor,
        ref spts,
    } = access_control(AccessControlRequest {
        program_id,
        signer_acc_info,
        registrar_acc_info,
        entity_acc_info,
        member_acc_info,
        vendor_acc_info,
        token_acc_info,
        cursor,
        spt_acc_infos,
    })?;

    Member::unpack_mut(
        &mut member_acc_info.try_borrow_mut_data()?,
        &mut |member: &mut Member| {
            state_transition(StateTransitionRequest {
                cursor,
                member,
                vendor,
                registrar_acc_info,
                vendor_acc_info,
                vendor_vault_authority_acc_info,
                vendor_vault_acc_info,
                token_acc_info,
                token_program_acc_info,
                spts,
            })
            .map_err(Into::into)
        },
    )?;

    Ok(())
}

fn access_control(req: AccessControlRequest) -> Result<AccessControlResponse, RegistryError> {
    msg!("access-control: claim_unlocked_reward");

    let AccessControlRequest {
        program_id,
        cursor,
        signer_acc_info,
        registrar_acc_info,
        entity_acc_info,
        member_acc_info,
        vendor_acc_info,
        token_acc_info,
        spt_acc_infos,
    } = req;

    // Authorization. Must be either the beneficiary or the node leader.
    if !signer_acc_info.is_signer {
        return Err(RegistryErrorCode::Unauthorized)?;
    }

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
        access_control::unlocked_reward_vendor(vendor_acc_info, registrar_acc_info, program_id)?;
    let _token = access_control::token(token_acc_info, &member.beneficiary)?;
    let is_mega = vendor.pool == registrar.pool_mint_mega;
    let spts = spt_acc_infos
        .iter()
        .enumerate()
        .map(|(idx, spt_acc_info)| {
            if is_mega {
                if &member.balances[idx].spt != spt_acc_info.key {
                    return Err(RegistryErrorCode::InvalidSpt)?;
                }
            } else {
                if &member.balances[idx].spt_mega != spt_acc_info.key {
                    return Err(RegistryErrorCode::InvalidSpt)?;
                }
            }
            access_control::token_account(spt_acc_info)
        })
        .collect::<Result<_, _>>()?;

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
    // Is the beneficiary or the node leader the signer?
    if &entity.leader != signer_acc_info.key && &member.beneficiary != signer_acc_info.key {
        return Err(RegistryErrorCode::Unauthorized)?;
    }

    Ok(AccessControlResponse { vendor, spts })
}

fn state_transition(req: StateTransitionRequest) -> Result<(), RegistryError> {
    msg!("state-transition: claim_unlocked_reward");

    let StateTransitionRequest {
        cursor,
        member,
        vendor,
        token_acc_info,
        registrar_acc_info,
        vendor_acc_info,
        vendor_vault_acc_info,
        vendor_vault_authority_acc_info,
        token_program_acc_info,
        spts,
    } = req;

    // Transfer proportion of the reward to the user.
    let spt_total = spts.iter().map(|a| a.amount).fold(0, |a, b| a + b);
    let amount = spt_total
        .checked_div(vendor.pool_token_supply)
        .unwrap()
        .checked_mul(vendor.total)
        .unwrap();

    let signer_seeds = &[
        registrar_acc_info.key.as_ref(),
        vendor_acc_info.key.as_ref(),
        &[vendor.nonce],
    ];
    invoke_token_transfer(
        vendor_vault_acc_info,
        token_acc_info,
        vendor_vault_authority_acc_info,
        token_program_acc_info,
        &[signer_seeds],
        amount,
    )?;

    // Move member rewards cursor.
    member.rewards_cursor = cursor + 1;

    Ok(())
}

struct AccessControlRequest<'a, 'b> {
    program_id: &'a Pubkey,
    cursor: u32,
    signer_acc_info: &'a AccountInfo<'b>,
    entity_acc_info: &'a AccountInfo<'b>,
    member_acc_info: &'a AccountInfo<'b>,
    registrar_acc_info: &'a AccountInfo<'b>,
    vendor_acc_info: &'a AccountInfo<'b>,
    token_acc_info: &'a AccountInfo<'b>,
    spt_acc_infos: Vec<&'a AccountInfo<'b>>,
}

struct AccessControlResponse {
    vendor: UnlockedRewardVendor,
    spts: Vec<TokenAccount>,
}

struct StateTransitionRequest<'a, 'b, 'c> {
    cursor: u32,
    member: &'c mut Member,
    vendor: &'c UnlockedRewardVendor,
    spts: &'c [TokenAccount],
    registrar_acc_info: &'a AccountInfo<'b>,
    vendor_acc_info: &'a AccountInfo<'b>,
    vendor_vault_authority_acc_info: &'a AccountInfo<'b>,
    vendor_vault_acc_info: &'a AccountInfo<'b>,
    token_program_acc_info: &'a AccountInfo<'b>,
    token_acc_info: &'a AccountInfo<'b>,
}
