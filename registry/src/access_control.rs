use crate::accounts::{vault, Entity, Member, Registrar};
use crate::error::{RegistryError, RegistryErrorCode};
use serum_common::pack::*;
use solana_client_gen::solana_sdk;
use solana_client_gen::solana_sdk::account_info::AccountInfo;
use solana_client_gen::solana_sdk::program_pack::Pack as TokenPack;
use solana_client_gen::solana_sdk::pubkey::Pubkey;
use solana_client_gen::solana_sdk::sysvar::clock::Clock;
use solana_client_gen::solana_sdk::sysvar::rent::Rent;
use solana_client_gen::solana_sdk::sysvar::Sysvar;
use spl_token::state::Account as TokenAccount;

pub fn governance(
    program_id: &Pubkey,
    registrar_acc_info: &AccountInfo,
    registrar_authority_acc_info: &AccountInfo,
) -> Result<Registrar, RegistryError> {
    if !registrar_authority_acc_info.is_signer {
        return Err(RegistryErrorCode::Unauthorized)?;
    }
    let r = registrar(registrar_acc_info, program_id)?;
    if r.authority != *registrar_authority_acc_info.key {
        return Err(RegistryErrorCode::Unauthorized)?;
    }
    Ok(r)
}
pub fn clock(acc_info: &AccountInfo) -> Result<Clock, RegistryError> {
    if *acc_info.key != solana_sdk::sysvar::clock::id() {
        return Err(RegistryErrorCode::InvalidClockSysvar)?;
    }
    Clock::from_account_info(acc_info).map_err(Into::into)
}

pub fn registrar(acc_info: &AccountInfo, program_id: &Pubkey) -> Result<Registrar, RegistryError> {
    if acc_info.owner != program_id {
        return Err(RegistryErrorCode::InvalidAccountOwner)?;
    }
    let registrar = Registrar::unpack(&acc_info.try_borrow_data()?)?;
    if !registrar.initialized {
        return Err(RegistryErrorCode::NotInitialized)?;
    }
    Ok(registrar)
}

pub fn entity(
    acc_info: &AccountInfo,
    registrar_acc_info: &AccountInfo,
    program_id: &Pubkey,
) -> Result<Entity, RegistryError> {
    if acc_info.owner != program_id {
        return Err(RegistryErrorCode::InvalidAccountOwner)?;
    }

    let e = Entity::unpack(&acc_info.try_borrow_data()?)?;
    if !e.initialized {
        return Err(RegistryErrorCode::NotInitialized)?;
    }
    if e.registrar != *registrar_acc_info.key {
        return Err(RegistryErrorCode::EntityRegistrarMismatch)?;
    }

    Ok(e)
}

pub fn member(
    acc_info: &AccountInfo,
    entity: &AccountInfo,
    beneficiary_acc_info: &AccountInfo,
    delegate_owner_acc_info: Option<&AccountInfo>,
    is_delegate: bool,
    program_id: &Pubkey,
) -> Result<Member, RegistryError> {
    if acc_info.owner != program_id {
        return Err(RegistryErrorCode::InvalidOwner)?;
    }
    let m = Member::unpack(&acc_info.try_borrow_data()?)?;
    if !m.initialized {
        return Err(RegistryErrorCode::NotInitialized)?;
    }
    if m.entity != *entity.key {
        return Err(RegistryErrorCode::MemberEntityMismatch)?;
    }
    if m.beneficiary != *beneficiary_acc_info.key {
        return Err(RegistryErrorCode::MemberBeneficiaryMismatch)?;
    }
    if is_delegate && *delegate_owner_acc_info.unwrap().key != m.books.delegate().owner {
        return Err(RegistryErrorCode::MemberDelegateMismatch)?;
    }
    Ok(m)
}

pub fn vault(
    acc_info: &AccountInfo,
    registrar_acc_info: &AccountInfo,
    registrar: &Registrar,
    program_id: &Pubkey,
    is_mega: bool,
) -> Result<TokenAccount, RegistryError> {
    if is_mega && registrar.mega_vault != *acc_info.key {
        return Err(RegistryErrorCode::RegistrarVaultMismatch)?;
    } else if !is_mega && registrar.vault != *acc_info.key {
        return Err(RegistryErrorCode::RegistrarVaultMismatch)?;
    }

    let vault = TokenAccount::unpack(&acc_info.try_borrow_data()?)?;

    let expected_vault_auth = Pubkey::create_program_address(
        &vault::signer_seeds(registrar_acc_info.key, &registrar.nonce),
        program_id,
    )
    .map_err(|_| RegistryErrorCode::InvalidVaultAuthority)?;
    if expected_vault_auth != vault.owner {
        return Err(RegistryErrorCode::InvalidVaultAuthority)?;
    }

    Ok(vault)
}

pub fn token(acc_info: &AccountInfo) -> Result<TokenAccount, RegistryError> {
    if *acc_info.owner != spl_token::ID {
        return Err(RegistryErrorCode::InvalidAccountOwner)?;
    }

    let token = TokenAccount::unpack(&acc_info.try_borrow_data()?)?;
    if token.state != spl_token::state::AccountState::Initialized {
        return Err(RegistryErrorCode::NotInitialized)?;
    }

    Ok(token)
}

pub fn rent(acc_info: &AccountInfo) -> Result<Rent, RegistryError> {
    if *acc_info.key != solana_sdk::sysvar::rent::id() {
        return Err(RegistryErrorCode::InvalidRentSysvar)?;
    }
    Rent::from_account_info(acc_info).map_err(Into::into)
}

pub fn vault_init(
    vault_acc_info: &AccountInfo,
    registrar_acc_info: &AccountInfo,
    rent: &Rent,
    nonce: u8,
    program_id: &Pubkey,
) -> Result<(), RegistryError> {
    let vault = token(vault_acc_info)?;
    let vault_authority = Pubkey::create_program_address(
        &vault::signer_seeds(registrar_acc_info.key, &nonce),
        program_id,
    )
    .map_err(|_| RegistryErrorCode::InvalidVaultNonce)?;

    if vault.owner != vault_authority {
        return Err(RegistryErrorCode::InvalidVaultAuthority)?;
    }
    // Rent.
    if !rent.is_exempt(vault_acc_info.lamports(), vault_acc_info.try_data_len()?) {
        return Err(RegistryErrorCode::NotRentExempt)?;
    }
    Ok(())
}
