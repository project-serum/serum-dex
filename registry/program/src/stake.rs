use serum_common::pack::Pack;
use serum_registry::accounts::{Entity, Member};
use serum_registry::error::{RegistryError, RegistryErrorCode};
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::info;
use solana_sdk::pubkey::Pubkey;

pub fn handler<'a>(
    program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'a>],
    amount: u64,
    is_mega: bool,
) -> Result<(), RegistryError> {
    info!("handler: stake");

    let acc_infos = &mut accounts.iter();

    let depositor_tok_owner_acc_info = next_account_info(acc_infos)?;
    let depositor_tok_acc_info = next_account_info(acc_infos)?;
    let member_acc_info = next_account_info(acc_infos)?;
    let member_authority_acc_info = next_account_info(acc_infos)?;
    let entity_acc_info = next_account_info(acc_infos)?;
    let token_program_acc_info = next_account_info(acc_infos)?;

    access_control(AccessControlRequest {
        depositor_tok_owner_acc_info,
        depositor_tok_acc_info,
        member_acc_info,
        member_authority_acc_info,
        entity_acc_info,
        token_program_acc_info,
    })?;

    Entity::unpack_mut(
        &mut entity_acc_info.try_borrow_mut_data()?,
        &mut |entity: &mut Entity| {
            Member::unpack_mut(
                &mut member_acc_info.try_borrow_mut_data()?,
                &mut |member: &mut Member| {
                    state_transition(StateTransitionRequest {
                        entity,
                        member,
                        amount,
                        is_mega,
                        depositor_tok_owner_acc_info,
                        depositor_tok_acc_info,
                        member_acc_info,
                        member_authority_acc_info,
                        entity_acc_info,
                        token_program_acc_info,
                    })
                    .map_err(Into::into)
                },
            )
        },
    )?;

    Ok(())
}

fn access_control(req: AccessControlRequest) -> Result<(), RegistryError> {
    info!("access-control: stake");

    let AccessControlRequest {
        depositor_tok_owner_acc_info,
        depositor_tok_acc_info,
        member_acc_info,
        member_authority_acc_info,
        entity_acc_info,
        token_program_acc_info,
    } = req;

    // todo

    info!("access-control: success");

    Ok(())
}

fn state_transition(req: StateTransitionRequest) -> Result<(), RegistryError> {
    info!("state-transition: stake");

    let StateTransitionRequest {
        entity,
        member,
        amount,
        is_mega,
        depositor_tok_owner_acc_info,
        depositor_tok_acc_info,
        member_acc_info,
        member_authority_acc_info,
        entity_acc_info,
        token_program_acc_info,
    } = req;

    // Transfer funds into the staking pool.
    {
        // todo
    }

    // Member account.
    {
        if is_mega {
            member.mega_amount += amount;
        } else {
            member.amount = amount;
        }
    }

    // Entity.
    {
        if is_mega {
            entity.mega_amount += amount;
        } else {
            entity.amount += amount;
        }
    }

    info!("state-transition: success");

    Ok(())
}

struct AccessControlRequest<'a> {
    depositor_tok_owner_acc_info: &'a AccountInfo<'a>,
    depositor_tok_acc_info: &'a AccountInfo<'a>,
    member_acc_info: &'a AccountInfo<'a>,
    member_authority_acc_info: &'a AccountInfo<'a>,
    entity_acc_info: &'a AccountInfo<'a>,
    token_program_acc_info: &'a AccountInfo<'a>,
}

struct StateTransitionRequest<'a, 'b> {
    entity: &'b mut Entity,
    member: &'b mut Member,
    amount: u64,
    is_mega: bool,
    depositor_tok_owner_acc_info: &'a AccountInfo<'a>,
    depositor_tok_acc_info: &'a AccountInfo<'a>,
    member_acc_info: &'a AccountInfo<'a>,
    member_authority_acc_info: &'a AccountInfo<'a>,
    entity_acc_info: &'a AccountInfo<'a>,
    token_program_acc_info: &'a AccountInfo<'a>,
}
