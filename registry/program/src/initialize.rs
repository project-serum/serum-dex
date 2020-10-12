use serum_common::pack::Pack;
use serum_registry::access_control;
use serum_registry::accounts::{registrar, vault, Registrar};
use serum_registry::error::{RegistryError, RegistryErrorCode};
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::info;
use solana_sdk::pubkey::Pubkey;

pub fn handler<'a>(
    program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'a>],
    authority: Pubkey,
    nonce: u8,
    withdrawal_timelock: i64,
    deactivation_timelock_premium: i64,
    reward_activation_threshold: u64,
) -> Result<(), RegistryError> {
    info!("handler: initialize");

    let acc_infos = &mut accounts.iter();

    let registrar_acc_info = next_account_info(acc_infos)?;
    let vault_acc_info = next_account_info(acc_infos)?;
    let mega_vault_acc_info = next_account_info(acc_infos)?;
    let rent_acc_info = next_account_info(acc_infos)?;

    access_control(AccessControlRequest {
        registrar_acc_info,
        rent_acc_info,
        vault_acc_info,
        mega_vault_acc_info,
        program_id,
        nonce,
    })?;

    Registrar::unpack_mut(
        &mut registrar_acc_info.try_borrow_mut_data()?,
        &mut |registrar: &mut Registrar| {
            state_transition(StateTransitionRequest {
                registrar,
                authority,
                vault_acc_info,
                mega_vault_acc_info,
                withdrawal_timelock,
                nonce,
                deactivation_timelock_premium,
                reward_activation_threshold,
            })
            .map_err(Into::into)
        },
    )?;

    Ok(())
}

fn access_control<'a>(req: AccessControlRequest<'a>) -> Result<(), RegistryError> {
    info!("access-control: initialize");

    let AccessControlRequest {
        registrar_acc_info,
        rent_acc_info,
        program_id,
        vault_acc_info,
        mega_vault_acc_info,
        nonce,
    } = req;

    // Authorization: none.

    // Account validation.
    let rent = access_control::rent(rent_acc_info)?;

    // Initialize specific.
    {
        // Registrar (uninitialized).
        {
            let registrar = Registrar::unpack(&registrar_acc_info.try_borrow_data()?)?;
            if registrar_acc_info.owner != program_id {
                return Err(RegistryErrorCode::InvalidOwner)?;
            }
            if !rent.is_exempt(
                registrar_acc_info.lamports(),
                registrar_acc_info.try_data_len()?,
            ) {
                return Err(RegistryErrorCode::NotRentExempt)?;
            }
            if registrar.initialized {
                return Err(RegistryErrorCode::AlreadyInitialized)?;
            }
        }

        // Vaults (initialized but not yet on the Registrar).
        access_control::vault_init(vault_acc_info, registrar_acc_info, &rent, nonce, program_id)?;
        access_control::vault_init(
            mega_vault_acc_info,
            registrar_acc_info,
            &rent,
            nonce,
            program_id,
        )?;
    }

    info!("access-control: success");

    Ok(())
}

fn state_transition(req: StateTransitionRequest) -> Result<(), RegistryError> {
    info!("state-transition: initialize");

    let StateTransitionRequest {
        registrar,
        authority,
        withdrawal_timelock,
        vault_acc_info,
        mega_vault_acc_info,
        nonce,
        deactivation_timelock_premium,
        reward_activation_threshold,
    } = req;

    registrar.initialized = true;
    registrar.capabilities_fees_bps = [0; 32];
    registrar.authority = authority;
    registrar.withdrawal_timelock = withdrawal_timelock;
    registrar.deactivation_timelock_premium = deactivation_timelock_premium;
    registrar.vault = *vault_acc_info.key;
    registrar.mega_vault = *mega_vault_acc_info.key;
    registrar.nonce = nonce;
    registrar.reward_activation_threshold = reward_activation_threshold;

    info!("state-transition: success");

    Ok(())
}

struct AccessControlRequest<'a> {
    registrar_acc_info: &'a AccountInfo<'a>,
    rent_acc_info: &'a AccountInfo<'a>,
    vault_acc_info: &'a AccountInfo<'a>,
    mega_vault_acc_info: &'a AccountInfo<'a>,
    program_id: &'a Pubkey,
    nonce: u8,
}

struct StateTransitionRequest<'a, 'b> {
    registrar: &'b mut Registrar,
    authority: Pubkey,
    withdrawal_timelock: i64,
    deactivation_timelock_premium: i64,
    nonce: u8,
    vault_acc_info: &'a AccountInfo<'a>,
    mega_vault_acc_info: &'a AccountInfo<'a>,
    reward_activation_threshold: u64,
}
