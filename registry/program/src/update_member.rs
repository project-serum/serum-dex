use serum_common::pack::*;
use serum_registry::accounts::{Member, Watchtower};
use serum_registry::error::{RegistryError, RegistryErrorCode};
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::info;
use solana_sdk::pubkey::Pubkey;
use std::convert::Into;

pub fn handler<'a>(
    program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'a>],
    watchtower: Option<Watchtower>,
    delegate: Option<Pubkey>,
) -> Result<(), RegistryError> {
    info!("handler: update_member");

    let acc_infos = &mut accounts.iter();

    let member_acc_info = next_account_info(acc_infos)?;
    let beneficiary_acc_info = next_account_info(acc_infos)?;

    access_control(AccessControlRequest {
        member_acc_info,
        beneficiary_acc_info,
        delegate,
        program_id,
    })?;

    Member::unpack_mut(
        &mut member_acc_info.try_borrow_mut_data()?,
        &mut |member: &mut Member| {
            state_transition(StateTransitionRequest {
                member,
                watchtower,
                delegate,
            })
            .map_err(Into::into)
        },
    )?;

    Ok(())
}

fn access_control(req: AccessControlRequest) -> Result<(), RegistryError> {
    // todo
    info!("access-control: update_member");

    let AccessControlRequest {
        member_acc_info,
        beneficiary_acc_info,
        delegate,
        program_id,
    } = req;

    // Beneficiary authorization.
    if !member_acc_info.is_signer {
        return Err(RegistryErrorCode::Unauthorized)?;
    }

    // Account validation.
    let member = Member::unpack(&member_acc_info.try_borrow_data()?)?;
    if !member.initialized {
        return Err(RegistryErrorCode::NotInitialized)?;
    }
    if member_acc_info.owner != program_id {
        return Err(RegistryErrorCode::InvalidAccountOwner)?;
    }
    if member.beneficiary != *member_acc_info.key {
        return Err(RegistryErrorCode::MemberBeneficiaryMismatch)?;
    }

    // UpdateMember specific.
    if delegate.is_some() {
        if !member.books.delegate().balances.is_empty() {
            return Err(RegistryErrorCode::DelegateInUse)?;
        }
    }

    info!("access-control: success");

    Ok(())
}

fn state_transition(req: StateTransitionRequest) -> Result<(), RegistryError> {
    // todo

    info!("state-transition: update_member");

    let StateTransitionRequest {
        member,
        watchtower,
        delegate,
    } = req;

    if let Some(wt) = watchtower {
        member.watchtower = wt;
    }
    if let Some(d) = delegate {
        member.set_delegate(d);
    }

    info!("state-transition: success");

    Ok(())
}

struct AccessControlRequest<'a> {
    member_acc_info: &'a AccountInfo<'a>,
    beneficiary_acc_info: &'a AccountInfo<'a>,
    program_id: &'a Pubkey,
    delegate: Option<Pubkey>,
}

struct StateTransitionRequest<'a> {
    member: &'a mut Member,
    watchtower: Option<Watchtower>,
    delegate: Option<Pubkey>,
}
