use crate::access_control;
use serum_common::pack::Pack;
use serum_common::program::invoke_token_transfer;
use serum_registry::accounts::{EntityState, Registrar};
use serum_registry_rewards::accounts::{vault, Instance};
use serum_registry_rewards::error::{RewardsError, RewardsErrorCode};
use solana_program::info;
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::pubkey::Pubkey;

pub fn handler(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    dex_instruction_data: Vec<u8>,
) -> Result<(), RewardsError> {
    info!("handler: crank_relay");

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

    let AccessControlResponse { instance } = access_control(AccessControlRequest {
        instance_acc_info,
        vault_acc_info,
        vault_authority_acc_info,
        registrar_acc_info,
        entity_acc_info,
        entity_leader_acc_info,
        dex_program_acc_info,
        event_q_acc_info,
        program_id,
        dex_instruction_data: &dex_instruction_data,
    })?;

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
        instance,
    })
}

fn access_control(req: AccessControlRequest) -> Result<AccessControlResponse, RewardsError> {
    info!("access-control: crank_relay");

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
        dex_instruction_data,
    } = req;

    // Entity leader authorization.
    if !entity_leader_acc_info.is_signer {
        return Err(RewardsErrorCode::Unauthorized)?;
    }

    // Account validation.
    let instance = access_control::instance(instance_acc_info, program_id)?;
    if &instance.registrar != registrar_acc_info.key {
        return Err(RewardsErrorCode::InvalidRegistrar)?;
    }
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
    let _v = access_control::vault(
        vault_acc_info,
        vault_authority_acc_info,
        &instance,
        instance_acc_info,
        program_id,
    )?;

    if entity.leader != *entity_leader_acc_info.key {
        return Err(RewardsErrorCode::InvalidLeader)?;
    }
    if entity.state != EntityState::Active {
        return Err(RewardsErrorCode::EntityNotActive)?;
    }

    if event_q_acc_info.owner != dex_program_acc_info.key {
        return Err(RewardsErrorCode::InvalidEventQueueOwner)?;
    }
    let version = dex_instruction_data[0];
    if version != 0 {
        return Err(RewardsErrorCode::InvalidDEXInstruction)?;
    }
    let mut ix_discrim_bytes = [0u8; 4];
    ix_discrim_bytes.copy_from_slice(&dex_instruction_data[1..5]);
    let ix_discrim = u32::from_le_bytes(ix_discrim_bytes);
    if ix_discrim != 3 {
        return Err(RewardsErrorCode::InvalidDEXInstruction)?;
    }

    Ok(AccessControlResponse { instance })
}

fn state_transition(req: StateTransitionRequest) -> Result<(), RewardsError> {
    info!("state-transition: crank_relay");

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
        instance,
    } = req;

    // Event queue len before.
    let before_event_count = event_q_len(&event_q_acc_info.try_borrow_data()?);

    // Invoke crank relay.
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

    // Event queue len after.
    let after_event_count = event_q_len(&event_q_acc_info.try_borrow_data()?);

    // Calculate payout amount.
    let amount = (before_event_count - after_event_count) * instance.fee_rate;

    // Pay out reward.
    invoke_token_transfer(
        vault_acc_info,
        token_acc_info,
        vault_authority_acc_info,
        token_program_acc_info,
        &[&vault::signer_seeds(instance_acc_info.key, &instance.nonce)],
        amount,
    )?;

    Ok(())
}

// Returns the length of the Serum DEX event queue account represented by the
// given `data`.
fn event_q_len(data: &[u8]) -> u64 {
    // b"serum" || account_flags || head.
    let count_start = 5 + 8 + 8;
    let count_end = count_start + 4;
    let mut b = [0u8; 4];
    b.copy_from_slice(&data[count_start..count_end]);
    u32::from_le_bytes(b) as u64
}

struct AccessControlRequest<'a, 'b> {
    instance_acc_info: &'a AccountInfo<'b>,
    vault_acc_info: &'a AccountInfo<'b>,
    vault_authority_acc_info: &'a AccountInfo<'b>,
    registrar_acc_info: &'a AccountInfo<'b>,
    entity_acc_info: &'a AccountInfo<'b>,
    entity_leader_acc_info: &'a AccountInfo<'b>,
    dex_program_acc_info: &'a AccountInfo<'b>,
    event_q_acc_info: &'a AccountInfo<'b>,
    program_id: &'a Pubkey,
    dex_instruction_data: &'a [u8],
}

struct AccessControlResponse {
    instance: Instance,
}

struct StateTransitionRequest<'a, 'b> {
    instance_acc_info: &'a AccountInfo<'b>,
    dex_program_acc_info: &'a AccountInfo<'b>,
    remaining_relay_accs: Vec<&'a AccountInfo<'b>>,
    event_q_acc_info: &'a AccountInfo<'b>,
    registrar_acc_info: &'a AccountInfo<'b>,
    dex_instruction_data: Vec<u8>,
    vault_acc_info: &'a AccountInfo<'b>,
    vault_authority_acc_info: &'a AccountInfo<'b>,
    token_acc_info: &'a AccountInfo<'b>,
    token_program_acc_info: &'a AccountInfo<'b>,
    instance: Instance,
}
