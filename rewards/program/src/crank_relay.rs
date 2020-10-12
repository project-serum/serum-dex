use crate::access_control;
use serum_common::pack::Pack;
use serum_registry::accounts::{EntityState, Registrar};
use serum_rewards::accounts::{vault, Instance};
use serum_rewards::error::{RewardsError, RewardsErrorCode};
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::info;
use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::pubkey::Pubkey;

pub fn handler<'a>(
    program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'a>],
    dex_instruction_data: Vec<u8>,
) -> Result<(), RewardsError> {
    info!("handler: exec");

    let acc_infos = &mut accounts.iter();

    let instance_acc_info = next_account_info(acc_infos)?;
    let vault_acc_info = next_account_info(acc_infos)?;
    let vault_authority_acc_info = next_account_info(acc_infos)?;
    let registrar_acc_info = next_account_info(acc_infos)?;
    let token_acc_info = next_account_info(acc_infos)?;
    let entity_acc_info = next_account_info(acc_infos)?;
    let entity_leader_acc_info = next_account_info(acc_infos)?;
    let token_program_acc_info = next_account_info(acc_infos)?;
    let dex_program_acc_info = next_account_info(acc_infos)?;
    let event_q_acc_info = next_account_info(acc_infos)?;

    // Relayed to the dex.
    let remaining_relay_accs: Vec<&AccountInfo> = acc_infos.collect();

    access_control(AccessControlRequest {
        instance_acc_info,
        vault_acc_info,
        vault_authority_acc_info,
        registrar_acc_info,
        entity_acc_info,
        entity_leader_acc_info,
        dex_program_acc_info,
        event_q_acc_info,
        program_id,
    })?;

    let instance = Instance::unpack(&instance_acc_info.try_borrow_data()?)?;
    state_transition(StateTransitionRequest {
        instance_acc_info,
        dex_program_acc_info,
        remaining_relay_accs,
        dex_instruction_data,
        event_q_acc_info,
        registrar_acc_info,
        vault_acc_info,
        vault_authority_acc_info,
        token_acc_info,
        token_program_acc_info,
        nonce: instance.nonce,
    })
}

fn access_control(req: AccessControlRequest) -> Result<(), RewardsError> {
    info!("access-control: exec");

    let AccessControlRequest {
        instance_acc_info,
        vault_acc_info,
        vault_authority_acc_info,
        registrar_acc_info,
        entity_acc_info,
        entity_leader_acc_info,
        dex_program_acc_info,
        event_q_acc_info,
        program_id,
    } = req;

    // Entity leader authorization.
    if !entity_leader_acc_info.is_signer {
        return Err(RewardsErrorCode::Unauthorized)?;
    }

    // Account validation.
    let instance = access_control::instance(instance_acc_info, program_id)?;
    let _ = serum_registry::access_control::registrar(
        registrar_acc_info,
        &instance.registry_program_id,
    )
    .map_err(|_| RewardsErrorCode::InvalidRegistrar)?;
    let entity = serum_registry::access_control::entity(
        entity_acc_info,
        registrar_acc_info,
        &instance.registry_program_id,
    )
    .map_err(|_| RewardsErrorCode::InvalidEntity)?;
    let _ = access_control::vault(
        vault_acc_info,
        vault_authority_acc_info,
        instance_acc_info,
        program_id,
    )?;
    if event_q_acc_info.owner != dex_program_acc_info.key {
        return Err(RewardsErrorCode::InvalidEventQueueOwner)?;
    }

    // Exec specific.
    if entity.leader != *entity_leader_acc_info.key {
        return Err(RewardsErrorCode::InvalidLeader)?;
    }
    // TODO: enable once pool is added to the registry.
    if false && entity.state != EntityState::Active {
        return Err(RewardsErrorCode::EntityNotActive)?;
    }

    info!("access-control: success");

    Ok(())
}

fn state_transition(req: StateTransitionRequest) -> Result<(), RewardsError> {
    info!("state-transition: exec");

    let StateTransitionRequest {
        instance_acc_info,
        dex_program_acc_info,
        remaining_relay_accs,
        dex_instruction_data,
        event_q_acc_info,
        registrar_acc_info,
        vault_acc_info,
        vault_authority_acc_info,
        token_acc_info,
        token_program_acc_info,
        nonce,
    } = req;

    // Event queue len before.
    let before_event_count = event_q_len(&event_q_acc_info.try_borrow_data()?);
    info!(&format!("before event account {:?}", before_event_count));

    // Invoke relay.
    {
        let relay_meta_accs = remaining_relay_accs
            .iter()
            .map(|acc_info| {
                if acc_info.is_writable {
                    AccountMeta::new(*acc_info.key, acc_info.is_signer)
                } else {
                    AccountMeta::new_readonly(*acc_info.key, acc_info.is_signer)
                }
            })
            .collect::<Vec<AccountMeta>>();
        let mut relay_accs = vec![dex_program_acc_info.clone()];
        for acc in remaining_relay_accs {
            relay_accs.push(acc.clone());
        }
        let dex_instruction = Instruction {
            program_id: *dex_program_acc_info.key,
            accounts: relay_meta_accs,
            data: dex_instruction_data,
        };
        solana_sdk::program::invoke(&dex_instruction, &relay_accs)?;
    }

    // Event queue len before.
    let after_event_count = event_q_len(&event_q_acc_info.try_borrow_data()?);
    info!(&format!("after event account {:?}", after_event_count));

    // Calculate payout amount.
    let amount = {
        let crank_capability_id = 0;
        let registrar = Registrar::unpack(&registrar_acc_info.try_borrow_data()?)?;
        let fee_rate_bps = registrar.fee_rate(crank_capability_id) as u64;
        let events_processed = before_event_count - after_event_count;
        events_processed * fee_rate_bps
    };

    // Pay out reward, if the vault has enough funds.
    {
        info!("invoking token transfer");
        let transfer_instruction = spl_token::instruction::transfer(
            &spl_token::ID,
            vault_acc_info.key,
            token_acc_info.key,
            vault_authority_acc_info.key,
            &[],
            amount.into(),
        )?;
        let signer_seeds = vault::signer_seeds(instance_acc_info.key, &nonce);
        solana_sdk::program::invoke_signed(
            &transfer_instruction,
            &[
                vault_acc_info.clone(),
                token_acc_info.clone(),
                vault_authority_acc_info.clone(),
                token_program_acc_info.clone(),
            ],
            &[&signer_seeds],
        )?;
    }

    info!("state-transition: success");

    Ok(())
}

// Returns the length of the Event queue account represented by the given `data`.
fn event_q_len(data: &[u8]) -> u64 {
    // b"serum" || account_flags || head.
    let count_start = 5 + 8 + 8;
    let count_end = count_start + 4;
    let mut b = [0u8; 4];
    b.copy_from_slice(&data[count_start..count_end]);
    u32::from_le_bytes(b) as u64
}

struct AccessControlRequest<'a> {
    instance_acc_info: &'a AccountInfo<'a>,
    vault_acc_info: &'a AccountInfo<'a>,
    vault_authority_acc_info: &'a AccountInfo<'a>,
    registrar_acc_info: &'a AccountInfo<'a>,
    entity_acc_info: &'a AccountInfo<'a>,
    entity_leader_acc_info: &'a AccountInfo<'a>,
    dex_program_acc_info: &'a AccountInfo<'a>,
    event_q_acc_info: &'a AccountInfo<'a>,
    program_id: &'a Pubkey,
}

struct StateTransitionRequest<'a> {
    instance_acc_info: &'a AccountInfo<'a>,
    dex_program_acc_info: &'a AccountInfo<'a>,
    remaining_relay_accs: Vec<&'a AccountInfo<'a>>,
    event_q_acc_info: &'a AccountInfo<'a>,
    registrar_acc_info: &'a AccountInfo<'a>,
    dex_instruction_data: Vec<u8>,
    vault_acc_info: &'a AccountInfo<'a>,
    vault_authority_acc_info: &'a AccountInfo<'a>,
    token_acc_info: &'a AccountInfo<'a>,
    token_program_acc_info: &'a AccountInfo<'a>,
    nonce: u8,
}
