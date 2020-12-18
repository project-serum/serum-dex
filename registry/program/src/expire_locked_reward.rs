use serum_common::pack::Pack;
use serum_common::program::invoke_token_transfer;
use serum_registry::access_control;
use serum_registry::accounts::LockedRewardVendor;
use serum_registry::error::{RegistryError, RegistryErrorCode};
use solana_program::msg;
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::pubkey::Pubkey;
use spl_token::state::Account as TokenAccount;

#[inline(never)]
pub fn handler(program_id: &Pubkey, accounts: &[AccountInfo]) -> Result<(), RegistryError> {
    msg!("handler: expire_locked_reward");

    let acc_infos = &mut accounts.iter();

    let expiry_receiver_acc_info = next_account_info(acc_infos)?;
    let token_acc_info = next_account_info(acc_infos)?;
    let vendor_acc_info = next_account_info(acc_infos)?;
    let vault_acc_info = next_account_info(acc_infos)?;
    let vault_authority_acc_info = next_account_info(acc_infos)?;
    let registrar_acc_info = next_account_info(acc_infos)?;
    let token_program_acc_info = next_account_info(acc_infos)?;
    let clock_acc_info = next_account_info(acc_infos)?;

    let AccessControlResponse { ref vault } = access_control(AccessControlRequest {
        program_id,
        registrar_acc_info,
        vendor_acc_info,
        vault_acc_info,
        token_acc_info,
        expiry_receiver_acc_info,
        clock_acc_info,
    })?;

    LockedRewardVendor::unpack_mut(
        &mut vendor_acc_info.try_borrow_mut_data()?,
        &mut |vendor: &mut LockedRewardVendor| {
            state_transition(StateTransitionRequest {
                vendor,
                vault,
                registrar_acc_info,
                vendor_acc_info,
                vault_acc_info,
                vault_authority_acc_info,
                token_acc_info,
                token_program_acc_info,
            })
            .map_err(Into::into)
        },
    )
    .map_err(Into::into)
}

fn access_control(req: AccessControlRequest) -> Result<AccessControlResponse, RegistryError> {
    msg!("access-control: expire_locked_reward");

    let AccessControlRequest {
        program_id,
        expiry_receiver_acc_info,
        registrar_acc_info,
        vault_acc_info,
        vendor_acc_info,
        token_acc_info,
        clock_acc_info,
    } = req;

    // Authorization.
    if !expiry_receiver_acc_info.is_signer {
        return Err(RegistryErrorCode::Unauthorized)?;
    }

    // Account validation.
    let _registrar = access_control::registrar(registrar_acc_info, program_id)?;
    let vendor =
        access_control::locked_reward_vendor(vendor_acc_info, registrar_acc_info, program_id)?;
    let vault = access_control::token_account(vault_acc_info)?;
    let token = access_control::token_account(token_acc_info)?;
    let clock = access_control::clock(clock_acc_info)?;

    if vendor.expired {
        return Err(RegistryErrorCode::VendorAlreadyExpired)?;
    }
    if &vendor.vault != vault_acc_info.key {
        return Err(RegistryErrorCode::InvalidVault)?;
    }
    if &vendor.expiry_receiver != expiry_receiver_acc_info.key {
        return Err(RegistryErrorCode::Unauthorized)?;
    }
    if &token.owner != expiry_receiver_acc_info.key {
        return Err(RegistryErrorCode::InvalidAccountOwner)?;
    }
    if clock.unix_timestamp < vendor.expiry_ts {
        return Err(RegistryErrorCode::VendorNotExpired)?;
    }

    Ok(AccessControlResponse { vault })
}

fn state_transition(req: StateTransitionRequest) -> Result<(), RegistryError> {
    msg!("state-transition: expire_locked_reward");

    let StateTransitionRequest {
        vendor,
        vault,
        token_acc_info,
        vendor_acc_info,
        vault_acc_info,
        vault_authority_acc_info,
        registrar_acc_info,
        token_program_acc_info,
    } = req;

    let signer_seeds = &[
        registrar_acc_info.key.as_ref(),
        vendor_acc_info.key.as_ref(),
        &[vendor.nonce],
    ];
    invoke_token_transfer(
        vault_acc_info,
        token_acc_info,
        vault_authority_acc_info,
        token_program_acc_info,
        &[signer_seeds],
        vault.amount,
    )?;

    vendor.expired = true;

    Ok(())
}

struct AccessControlRequest<'a, 'b> {
    program_id: &'a Pubkey,
    expiry_receiver_acc_info: &'a AccountInfo<'b>,
    registrar_acc_info: &'a AccountInfo<'b>,
    vendor_acc_info: &'a AccountInfo<'b>,
    vault_acc_info: &'a AccountInfo<'b>,
    token_acc_info: &'a AccountInfo<'b>,
    clock_acc_info: &'a AccountInfo<'b>,
}

struct AccessControlResponse {
    vault: TokenAccount,
}

struct StateTransitionRequest<'a, 'b, 'c> {
    vendor: &'c mut LockedRewardVendor,
    vault: &'c TokenAccount,
    registrar_acc_info: &'a AccountInfo<'b>,
    vendor_acc_info: &'a AccountInfo<'b>,
    vault_authority_acc_info: &'a AccountInfo<'b>,
    vault_acc_info: &'a AccountInfo<'b>,
    token_program_acc_info: &'a AccountInfo<'b>,
    token_acc_info: &'a AccountInfo<'b>,
}
