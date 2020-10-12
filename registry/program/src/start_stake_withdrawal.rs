use serum_common::pack::Pack;
use serum_registry::access_control;
use serum_registry::accounts::{Entity, Member, PendingWithdrawal, Registrar};
use serum_registry::error::{RegistryError, RegistryErrorCode};
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::info;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::sysvar::clock::Clock;

pub fn handler<'a>(
    program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'a>],
    amount: u64,
    mega: bool,
    delegate: bool,
) -> Result<(), RegistryError> {
    info!("handler: initiate_stake_withdrawal");

    let acc_infos = &mut accounts.iter();

    let pending_withdrawal_acc_info = next_account_info(acc_infos)?;
    let beneficiary_acc_info = next_account_info(acc_infos)?;
    let member_acc_info = next_account_info(acc_infos)?;
    let entity_acc_info = next_account_info(acc_infos)?;
    let registrar_acc_info = next_account_info(acc_infos)?;
    let rent_acc_info = next_account_info(acc_infos)?;
    let clock_acc_info = next_account_info(acc_infos)?;

    let delegate_owner_acc_info = {
        if delegate {
            Some(next_account_info(acc_infos)?)
        } else {
            None
        }
    };

    access_control(AccessControlRequest {
        pending_withdrawal_acc_info,
        beneficiary_acc_info,
        registrar_acc_info,
        member_acc_info,
        delegate_owner_acc_info,
        entity_acc_info,
        rent_acc_info,
        clock_acc_info,
        program_id,
        delegate,
    })?;

    PendingWithdrawal::unpack_mut(
        &mut pending_withdrawal_acc_info.try_borrow_mut_data()?,
        &mut |pending_withdrawal: &mut PendingWithdrawal| {
            Entity::unpack_mut(
                &mut entity_acc_info.try_borrow_mut_data()?,
                &mut |entity: &mut Entity| {
                    Member::unpack_mut(
                        &mut member_acc_info.try_borrow_mut_data()?,
                        &mut |member: &mut Member| {
                            let registrar =
                                Registrar::unpack(&registrar_acc_info.try_borrow_data()?)?;
                            let clock = access_control::clock(clock_acc_info)?;
                            state_transition(StateTransitionRequest {
                                pending_withdrawal,
                                registrar,
                                member,
                                entity,
                                member_acc_info,
                                clock,
                                mega,
                                delegate,
                                amount,
                            })
                            .map_err(Into::into)
                        },
                    )
                },
            )
        },
    )?;

    Ok(())
}

fn access_control(req: AccessControlRequest) -> Result<(), RegistryError> {
    info!("access-control: initiate_stake_withdrawal");

    let AccessControlRequest {
        registrar_acc_info,
        pending_withdrawal_acc_info,
        beneficiary_acc_info,
        member_acc_info,
        entity_acc_info,
        delegate_owner_acc_info,
        rent_acc_info,
        clock_acc_info,
        program_id,
        delegate,
    } = req;

    // Beneficiary/delegate authorization.
    if !beneficiary_acc_info.is_signer {
        return Err(RegistryErrorCode::Unauthorized)?;
    }

    // Account validation.
    let rent = access_control::rent(rent_acc_info)?;
    let _ = access_control::clock(clock_acc_info)?;
    let _ = access_control::registrar(registrar_acc_info, program_id)?;
    let _ = access_control::entity(entity_acc_info, registrar_acc_info, program_id)?;
    let _ = access_control::member(
        member_acc_info,
        entity_acc_info,
        beneficiary_acc_info,
        delegate_owner_acc_info,
        delegate,
        program_id,
    )?;

    // StartStakeWithdrawal specific.
    {
        let pw = PendingWithdrawal::unpack(&pending_withdrawal_acc_info.try_borrow_data()?)?;
        if pending_withdrawal_acc_info.owner != program_id {
            return Err(RegistryErrorCode::InvalidAccountOwner)?;
        }
        if pw.initialized {
            return Err(RegistryErrorCode::AlreadyInitialized)?;
        }
        // TODO: this doesn't actually need to be rent exempt, since the account
        //       only needs to live during the pending withdrawal window.
        if !rent.is_exempt(
            pending_withdrawal_acc_info.lamports(),
            pending_withdrawal_acc_info.try_data_len()?,
        ) {
            return Err(RegistryErrorCode::NotRentExempt)?;
        }
    }

    info!("access-control: success");

    Ok(())
}

fn state_transition(req: StateTransitionRequest) -> Result<(), RegistryError> {
    info!("state-transition: initiate_stake_withdrawal");

    let StateTransitionRequest {
        pending_withdrawal,
        registrar,
        entity,
        member,
        member_acc_info,
        clock,
        amount,
        delegate,
        mega,
    } = req;

    // TODO: we should probably burn the stake pool token here.

    // Print the pending withdrawal receipt.
    pending_withdrawal.initialized = true;
    pending_withdrawal.member = *member_acc_info.key;
    pending_withdrawal.start_ts = clock.unix_timestamp;
    pending_withdrawal.end_ts = clock.unix_timestamp + registrar.deactivation_timelock();
    pending_withdrawal.amount = amount;
    pending_withdrawal.delegate = delegate;
    pending_withdrawal.mega = mega;

    // Bookeeping.
    entity.transfer_pending_withdrawal(amount, mega, &registrar, &clock);
    member.transfer_pending_withdrawal(amount, mega, delegate);

    info!("state-transition: success");

    Ok(())
}

struct AccessControlRequest<'a> {
    registrar_acc_info: &'a AccountInfo<'a>,
    pending_withdrawal_acc_info: &'a AccountInfo<'a>,
    beneficiary_acc_info: &'a AccountInfo<'a>,
    member_acc_info: &'a AccountInfo<'a>,
    delegate_owner_acc_info: Option<&'a AccountInfo<'a>>,
    entity_acc_info: &'a AccountInfo<'a>,
    rent_acc_info: &'a AccountInfo<'a>,
    clock_acc_info: &'a AccountInfo<'a>,
    program_id: &'a Pubkey,
    delegate: bool,
}

struct StateTransitionRequest<'a, 'b> {
    pending_withdrawal: &'b mut PendingWithdrawal,
    entity: &'b mut Entity,
    member: &'b mut Member,
    registrar: Registrar,
    member_acc_info: &'a AccountInfo<'a>,
    clock: Clock,
    amount: u64,
    delegate: bool,
    mega: bool,
}
