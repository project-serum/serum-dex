use serum_common::pack::Pack;
use serum_common::program::invoke_token_transfer;
use serum_registry::access_control;
use serum_registry::accounts::{EntityState, Member, Registrar, UnlockedRewardVendor};
use serum_registry::error::{RegistryError, RegistryErrorCode};
use solana_program::info;
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::pubkey::Pubkey;

#[inline(never)]
pub fn handler(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    cursor: u32,
) -> Result<(), RegistryError> {
    info!("handler: claim_unlocked_reward");

    let acc_infos = &mut accounts.iter();

    let entity_acc_info = next_account_info(acc_infos)?;
    let member_acc_info = next_account_info(acc_infos)?;
    let registrar_acc_info = next_account_info(acc_infos)?;
    let vendor_acc_info = next_account_info(acc_infos)?;
    let vendor_vault_acc_info = next_account_info(acc_infos)?;
    let vendor_vault_authority_acc_info = next_account_info(acc_infos)?;
    let token_acc_info = next_account_info(acc_infos)?;
    let token_program_acc_info = next_account_info(acc_infos)?;

    let AccessControlResponse {
        ref registrar,
        ref vendor,
    } = access_control(AccessControlRequest {
        program_id,
        registrar_acc_info,
        entity_acc_info,
        member_acc_info,
        vendor_acc_info,
        token_acc_info,
        cursor,
    })?;

    Member::unpack_mut(
        &mut member_acc_info.try_borrow_mut_data()?,
        &mut |member: &mut Member| {
            state_transition(StateTransitionRequest {
                cursor,
                member,
                vendor,
                registrar,
                registrar_acc_info,
                vendor_acc_info,
                vendor_vault_authority_acc_info,
                vendor_vault_acc_info,
                token_acc_info,
                token_program_acc_info,
            })
            .map_err(Into::into)
        },
    )?;

    Ok(())
}

fn access_control(req: AccessControlRequest) -> Result<AccessControlResponse, RegistryError> {
    info!("access-control: claim_unlocked_reward");

    let AccessControlRequest {
        program_id,
        cursor,
        registrar_acc_info,
        entity_acc_info,
        member_acc_info,
        vendor_acc_info,
        token_acc_info,
    } = req;

    // Authorization: none required. This operation is purely beneficial for
    //                the member account, the beneficiary of which must own
    //                the destination token to which rewards are sent.

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
    info!("state-transition: claim_unlocked_reward");

    let StateTransitionRequest {
        cursor,
        member,
        vendor,
        token_acc_info,
        registrar,
        registrar_acc_info,
        vendor_acc_info,
        vendor_vault_acc_info,
        vendor_vault_authority_acc_info,
        token_program_acc_info,
    } = req;

    // Transfer proportion of the reward to the user.
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
    entity_acc_info: &'a AccountInfo<'b>,
    member_acc_info: &'a AccountInfo<'b>,
    registrar_acc_info: &'a AccountInfo<'b>,
    vendor_acc_info: &'a AccountInfo<'b>,
    token_acc_info: &'a AccountInfo<'b>,
}

struct AccessControlResponse {
    vendor: UnlockedRewardVendor,
    registrar: Registrar,
}

struct StateTransitionRequest<'a, 'b, 'c> {
    cursor: u32,
    member: &'c mut Member,
    vendor: &'c UnlockedRewardVendor,
    registrar: &'c Registrar,
    registrar_acc_info: &'a AccountInfo<'b>,
    vendor_acc_info: &'a AccountInfo<'b>,
    vendor_vault_authority_acc_info: &'a AccountInfo<'b>,
    vendor_vault_acc_info: &'a AccountInfo<'b>,
    token_program_acc_info: &'a AccountInfo<'b>,
    token_acc_info: &'a AccountInfo<'b>,
}
