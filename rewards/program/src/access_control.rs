use serum_common::pack::Pack;
use serum_rewards::accounts::{vault, Instance};
use serum_rewards::error::{RewardsError, RewardsErrorCode};
use solana_sdk::account_info::AccountInfo;
use solana_sdk::program_pack::Pack as TokenPack;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::sysvar::rent::Rent;
use solana_sdk::sysvar::Sysvar;
use spl_token::state::Account as TokenAccount;

pub fn governance(
    program_id: &Pubkey,
    instance_acc_info: &AccountInfo,
    instance_authority_acc_info: &AccountInfo,
) -> Result<Instance, RewardsError> {
    if !instance_authority_acc_info.is_signer {
        return Err(RewardsErrorCode::Unauthorized)?;
    }
    let instance = instance(instance_acc_info, program_id)?;
    if instance.authority != *instance_authority_acc_info.key {
        return Err(RewardsErrorCode::Unauthorized)?;
    }
    Ok(instance)
}

pub fn rent(acc_info: &AccountInfo) -> Result<Rent, RewardsError> {
    if *acc_info.key != solana_sdk::sysvar::rent::id() {
        return Err(RewardsErrorCode::InvalidRentSysvar)?;
    }
    Rent::from_account_info(acc_info).map_err(Into::into)
}

pub fn token(acc_info: &AccountInfo) -> Result<TokenAccount, RewardsError> {
    if *acc_info.owner != spl_token::ID {
        return Err(RewardsErrorCode::InvalidAccountOwner)?;
    }

    let token = TokenAccount::unpack(&acc_info.try_borrow_data()?)?;
    if token.state != spl_token::state::AccountState::Initialized {
        return Err(RewardsErrorCode::NotInitialized)?;
    }

    Ok(token)
}

pub fn vault_init(
    vault_acc_info: &AccountInfo,
    instance_acc_info: &AccountInfo,
    rent: &Rent,
    nonce: u8,
    program_id: &Pubkey,
) -> Result<(), RewardsError> {
    let vault = token(vault_acc_info)?;
    let vault_authority = Pubkey::create_program_address(
        &vault::signer_seeds(instance_acc_info.key, &nonce),
        program_id,
    )
    .map_err(|_| RewardsErrorCode::InvalidVaultNonce)?;

    if vault.owner != vault_authority {
        return Err(RewardsErrorCode::InvalidVaultAuthority)?;
    }
    // Rent.
    if !rent.is_exempt(vault_acc_info.lamports(), vault_acc_info.try_data_len()?) {
        return Err(RewardsErrorCode::NotRentExempt)?;
    }
    Ok(())
}

pub fn vault(
    acc_info: &AccountInfo,
    vault_authority_acc_info: &AccountInfo,
    instance_acc_info: &AccountInfo,
    program_id: &Pubkey,
) -> Result<TokenAccount, RewardsError> {
    let instance = instance(instance_acc_info, program_id)?;
    let vault = token(acc_info)?;
    if *acc_info.key != instance.vault {
        return Err(RewardsErrorCode::InvalidVault)?;
    }

    let va = vault_authority(
        vault_authority_acc_info,
        instance_acc_info.key,
        &instance,
        program_id,
    )?;

    if va != vault.owner {
        return Err(RewardsErrorCode::InvalidVault)?;
    }
    if va != *vault_authority_acc_info.key {
        return Err(RewardsErrorCode::InvalidVault)?;
    }

    Ok(vault)
}

fn vault_authority(
    vault_authority_acc_info: &AccountInfo,
    instance_addr: &Pubkey,
    instance: &Instance,
    program_id: &Pubkey,
) -> Result<Pubkey, RewardsError> {
    let va = Pubkey::create_program_address(
        &vault::signer_seeds(instance_addr, &instance.nonce),
        program_id,
    )
    .map_err(|_| RewardsErrorCode::InvalidVaultNonce)?;
    if va != *vault_authority_acc_info.key {
        return Err(RewardsErrorCode::InvalidVaultAuthority)?;
    }

    Ok(va)
}

pub fn instance(
    instance_acc_info: &AccountInfo,
    program_id: &Pubkey,
) -> Result<Instance, RewardsError> {
    if instance_acc_info.owner != program_id {
        return Err(RewardsErrorCode::InvalidAccountOwner)?;
    }
    let instance = Instance::unpack(&instance_acc_info.try_borrow_data()?)?;
    if !instance.initialized {
        return Err(RewardsErrorCode::NotInitialized)?;
    }
    Ok(instance)
}
