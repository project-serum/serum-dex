use serum_common::pack::Pack;
use serum_registry::access_control;
use serum_registry::accounts::{Member, MemberBooks, Watchtower};
use serum_registry::error::{RegistryError, RegistryErrorCode};
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::info;
use solana_sdk::pubkey::Pubkey;

pub fn handler<'a>(
    program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'a>],
    beneficiary: Pubkey,
    delegate: Pubkey,
    watchtower: Watchtower,
) -> Result<(), RegistryError> {
    info!("handler: join_entity");

    let acc_infos = &mut accounts.iter();

    let member_acc_info = next_account_info(acc_infos)?;
    let entity_acc_info = next_account_info(acc_infos)?;
    let registrar_acc_info = next_account_info(acc_infos)?;
    let rent_acc_info = next_account_info(acc_infos)?;

    access_control(AccessControlRequest {
        member_acc_info,
        entity_acc_info,
        registrar_acc_info,
        rent_acc_info,
        program_id,
    })?;

    Member::unpack_mut(
        &mut member_acc_info.try_borrow_mut_data()?,
        &mut |member: &mut Member| {
            state_transition(StateTransitionRequest {
                member,
                beneficiary,
                delegate,
                entity_acc_info,
                registrar_acc_info,
                watchtower,
            })
            .map_err(Into::into)
        },
    )?;

    Ok(())
}

fn access_control(req: AccessControlRequest) -> Result<(), RegistryError> {
    info!("access-control: join_entity");

    let AccessControlRequest {
        member_acc_info,
        entity_acc_info,
        rent_acc_info,
        registrar_acc_info,
        program_id,
    } = req;

    // Authorization: none.

    // Account validation.
    let rent = access_control::rent(rent_acc_info)?;
    let registrar = access_control::registrar(registrar_acc_info, program_id)?;
    let entity = access_control::entity(entity_acc_info, registrar_acc_info, program_id)?;

    // JoinEntity checks.
    {
        let member = Member::unpack(&member_acc_info.try_borrow_data()?)?;
        if member_acc_info.owner != program_id {
            return Err(RegistryErrorCode::InvalidAccountOwner)?;
        }
        if member.initialized {
            return Err(RegistryErrorCode::AlreadyInitialized)?;
        }
        if !rent.is_exempt(member_acc_info.lamports(), member_acc_info.try_data_len()?) {
            return Err(RegistryErrorCode::NotRentExempt)?;
        }
    }

    info!("access-control: success");

    Ok(())
}

fn state_transition(req: StateTransitionRequest) -> Result<(), RegistryError> {
    info!("state-transition: join_entity");

    let StateTransitionRequest {
        member,
        beneficiary,
        delegate,
        entity_acc_info,
        registrar_acc_info,
        watchtower,
    } = req;

    member.initialized = true;
    member.registrar = *registrar_acc_info.key;
    member.entity = *entity_acc_info.key;
    member.beneficiary = beneficiary;
    member.generation = 0;
    member.watchtower = watchtower;
    member.books = MemberBooks::new(beneficiary, delegate);

    info!("state-transition: success");

    Ok(())
}

struct AccessControlRequest<'a> {
    member_acc_info: &'a AccountInfo<'a>,
    entity_acc_info: &'a AccountInfo<'a>,
    registrar_acc_info: &'a AccountInfo<'a>,
    rent_acc_info: &'a AccountInfo<'a>,
    program_id: &'a Pubkey,
}

struct StateTransitionRequest<'a, 'b> {
    member: &'b mut Member,
    beneficiary: Pubkey,
    delegate: Pubkey,
    entity_acc_info: &'a AccountInfo<'a>,
    registrar_acc_info: &'a AccountInfo<'a>,
    watchtower: Watchtower,
}
