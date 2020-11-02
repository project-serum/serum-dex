use serum_common::pack::*;
use serum_registry::access_control;
use serum_registry::accounts::Member;
use serum_registry::error::{RegistryError, RegistryErrorCode};
use solana_program::info;
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::pubkey::Pubkey;
use std::convert::Into;

#[inline(never)]
pub fn handler(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    delegate: Option<Pubkey>,
    metadata: Option<Pubkey>,
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
                delegate,
                metadata,
            })
            .map_err(Into::into)
        },
    )?;

    Ok(())
}

fn access_control(req: AccessControlRequest) -> Result<(), RegistryError> {
    info!("access-control: update_member");

    let AccessControlRequest {
        member_acc_info,
        beneficiary_acc_info,
        delegate,
        program_id,
    } = req;

    // Beneficiary authorization.
    if !beneficiary_acc_info.is_signer {
        return Err(RegistryErrorCode::Unauthorized)?;
    }

    // Account validation.
    let member = access_control::member(member_acc_info, beneficiary_acc_info, program_id)?;

    // UpdateMember specific.
    if delegate.is_some() {
        // Can't overwrite the delegate if we haven't returned it's deposit.
        if !member.balances.delegate.is_empty() {
            return Err(RegistryErrorCode::DelegateInUse)?;
        }
    }

    Ok(())
}

fn state_transition(req: StateTransitionRequest) -> Result<(), RegistryError> {
    info!("state-transition: update_member");

    let StateTransitionRequest {
        member,
        delegate,
        metadata,
    } = req;

    if let Some(d) = delegate {
        member.set_delegate(d);
    }
    if let Some(m) = metadata {
        member.metadata = m;
    }

    Ok(())
}

struct AccessControlRequest<'a, 'b> {
    member_acc_info: &'a AccountInfo<'b>,
    beneficiary_acc_info: &'a AccountInfo<'b>,
    program_id: &'a Pubkey,
    delegate: Option<Pubkey>,
}

struct StateTransitionRequest<'a> {
    member: &'a mut Member,
    delegate: Option<Pubkey>,
    metadata: Option<Pubkey>,
}
