use serum_common::pack::Pack;
use serum_registry::accounts::Member;
use serum_registry::error::{RegistryError, RegistryErrorCode};
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::info;
use solana_sdk::pubkey::Pubkey;

pub fn handler<'a>(
    program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'a>],
    beneficiary: Pubkey,
    delegate: Pubkey,
) -> Result<(), RegistryError> {
    info!("handler: join_entity");

    let acc_infos = &mut accounts.iter();

    let member_acc_info = next_account_info(acc_infos)?;
    let entity_acc_info = next_account_info(acc_infos)?;
    let rent_acc_info = next_account_info(acc_infos)?;

    access_control(AccessControlRequest {
        member_acc_info,
        entity_acc_info,
        rent_acc_info,
    })?;

    Member::unpack_mut(
        &mut member_acc_info.try_borrow_mut_data()?,
        &mut |member: &mut Member| {
            state_transition(StateTransitionRequest {
                member,
                beneficiary,
                delegate,
                entity_acc_info,
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
    } = req;

    // todo

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
    } = req;

    member.initialized = true;
    member.entity = *entity_acc_info.key;
    member.beneficiary = beneficiary;
    member.delegate = delegate;
    member.amount = 0;
    member.mega_amount = 0;

    info!("state-transition: success");

    Ok(())
}

struct AccessControlRequest<'a> {
    member_acc_info: &'a AccountInfo<'a>,
    entity_acc_info: &'a AccountInfo<'a>,
    rent_acc_info: &'a AccountInfo<'a>,
}

struct StateTransitionRequest<'a, 'b> {
    member: &'b mut Member,
    beneficiary: Pubkey,
    delegate: Pubkey,
    entity_acc_info: &'a AccountInfo<'a>,
}
