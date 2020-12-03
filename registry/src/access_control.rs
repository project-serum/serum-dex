use crate::accounts::reward_queue::Ring;
use crate::accounts::{
    vault, Entity, LockedRewardVendor, Member, PendingWithdrawal, Registrar, RewardEventQueue,
    UnlockedRewardVendor,
};
use crate::error::{RegistryError, RegistryErrorCode};
use serum_common::pack::*;
use solana_client_gen::solana_sdk;
use solana_client_gen::solana_sdk::account_info::AccountInfo;
use solana_client_gen::solana_sdk::program_pack::Pack as TokenPack;
use solana_client_gen::solana_sdk::pubkey::Pubkey;
use solana_client_gen::solana_sdk::sysvar::clock::Clock;
use solana_client_gen::solana_sdk::sysvar::rent::Rent;
use solana_client_gen::solana_sdk::sysvar::Sysvar;
use spl_token::state::{Account as TokenAccount, Mint};

#[inline]
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
    let mut data: &[u8] = &acc_info.try_borrow_data()?;
    let e = Entity::unpack_unchecked(&mut data)?;
    entity_check(&e, acc_info, registrar_acc_info, program_id)?;
    Ok(e)
}

pub fn entity_check(
    entity: &Entity,
    acc_info: &AccountInfo,
    registrar_acc_info: &AccountInfo,
    program_id: &Pubkey,
) -> Result<(), RegistryError> {
    if acc_info.owner != program_id {
        return Err(RegistryErrorCode::InvalidAccountOwner)?;
    }
    if !entity.initialized {
        return Err(RegistryErrorCode::NotInitialized)?;
    }
    if entity.registrar != *registrar_acc_info.key {
        return Err(RegistryErrorCode::EntityRegistrarMismatch)?;
    }

    Ok(())
}

pub fn delegate_check(
    member: &Member,
    delegate_owner_acc_info: Option<&AccountInfo>,
    is_delegate: bool,
) -> Result<(), RegistryError> {
    if is_delegate && *delegate_owner_acc_info.unwrap().key != member.balances.delegate.owner {
        return Err(RegistryErrorCode::MemberDelegateMismatch)?;
    }
    Ok(())
}

pub fn member(
    acc_info: &AccountInfo,
    beneficiary_acc_info: &AccountInfo,
    program_id: &Pubkey,
) -> Result<Member, RegistryError> {
    if acc_info.owner != program_id {
        return Err(RegistryErrorCode::InvalidOwner)?;
    }

    let m = Member::unpack(&acc_info.try_borrow_data()?)?;
    if !m.initialized {
        return Err(RegistryErrorCode::NotInitialized)?;
    }
    if m.beneficiary != *beneficiary_acc_info.key {
        return Err(RegistryErrorCode::MemberBeneficiaryMismatch)?;
    }
    Ok(m)
}

pub fn member_account(
    acc_info: &AccountInfo,
    registrar_acc_info: &AccountInfo,
    program_id: &Pubkey,
) -> Result<Member, RegistryError> {
    if acc_info.owner != program_id {
        return Err(RegistryErrorCode::InvalidOwner)?;
    }

    let m = Member::unpack(&acc_info.try_borrow_data()?)?;
    if !m.initialized {
        return Err(RegistryErrorCode::NotInitialized)?;
    }
    if &m.registrar != registrar_acc_info.key {
        return Err(RegistryErrorCode::MemberRegistrarMismatch)?;
    }
    Ok(m)
}

pub fn member_belongs_to(
    acc_info: &AccountInfo,
    registrar_acc_info: &AccountInfo,
    entity_acc_info: &AccountInfo,
    program_id: &Pubkey,
) -> Result<Member, RegistryError> {
    if acc_info.owner != program_id {
        return Err(RegistryErrorCode::InvalidOwner)?;
    }

    let m = Member::unpack(&acc_info.try_borrow_data()?)?;
    if !m.initialized {
        return Err(RegistryErrorCode::NotInitialized)?;
    }
    if &m.registrar != registrar_acc_info.key {
        return Err(RegistryErrorCode::MemberRegistrarMismatch)?;
    }
    if &m.entity != entity_acc_info.key {
        return Err(RegistryErrorCode::MemberEntityMismatch)?;
    }

    Ok(m)
}

pub fn member_join(
    acc_info: &AccountInfo,
    entity: &AccountInfo,
    beneficiary_acc_info: &AccountInfo,
    program_id: &Pubkey,
) -> Result<Member, RegistryError> {
    let m = member(acc_info, beneficiary_acc_info, program_id)?;

    if m.entity != *entity.key {
        return Err(RegistryErrorCode::MemberEntityMismatch)?;
    }
    Ok(m)
}

#[inline(always)]
pub fn member_raw(
    acc_info: &AccountInfo,
    entity_acc_info: &AccountInfo,
    program_id: &Pubkey,
) -> Result<Member, RegistryError> {
    if acc_info.owner != program_id {
        return Err(RegistryErrorCode::InvalidOwner)?;
    }

    let m = Member::unpack(&acc_info.try_borrow_data()?)?;
    if !m.initialized {
        return Err(RegistryErrorCode::NotInitialized)?;
    }
    if m.entity != *entity_acc_info.key {
        return Err(RegistryErrorCode::MemberEntityMismatch)?;
    }
    Ok(m)
}

pub fn reward_event_q<'a, 'b, 'c>(
    reward_event_q_acc_info: &'a AccountInfo<'b>,
    registrar_acc_info: &'a AccountInfo<'b>,
    registrar: &'c Registrar,
    program_id: &'a Pubkey,
) -> Result<RewardEventQueue<'b>, RegistryError> {
    if reward_event_q_acc_info.owner == program_id {
        return Err(RegistryErrorCode::InvalidOwner)?;
    }
    if registrar.reward_event_q != *reward_event_q_acc_info.key {
        return Err(RegistryErrorCode::RegistrarRewardQMismatch)?;
    }
    let q = RewardEventQueue::from(reward_event_q_acc_info.data.clone());
    if q.authority() != *registrar_acc_info.key {
        return Err(RegistryErrorCode::RegistrarRewardQMismatch)?;
    }
    Ok(q)
}

pub fn pool_vault(
    pool_vault_acc_info: &AccountInfo,
    registrar: &Registrar,
) -> Result<(TokenAccount, bool), RegistryError> {
    let v = token_account(pool_vault_acc_info)?;

    if pool_vault_acc_info.key != &registrar.pool_vault
        && pool_vault_acc_info.key != &registrar.pool_vault_mega
    {
        return Err(RegistryErrorCode::InvalidVault)?;
    }

    let is_mega = pool_vault_acc_info.key == &registrar.pool_vault_mega;

    Ok((v, is_mega))
}

pub fn pool_mint(
    pool_mint_acc_info: &AccountInfo,
    registrar: &Registrar,
    is_mega: bool,
) -> Result<Mint, RegistryError> {
    if is_mega {
        if pool_mint_acc_info.key != &registrar.pool_mint_mega {
            return Err(RegistryErrorCode::InvalidPoolTokenMint)?;
        }
    } else {
        if pool_mint_acc_info.key != &registrar.pool_mint {
            return Err(RegistryErrorCode::InvalidPoolTokenMint)?;
        }
    }

    let mint = mint(pool_mint_acc_info)?;

    Ok(mint)
}

pub fn pool_token(
    pool_token_acc_info: &AccountInfo,
    pool_mint_acc_info: &AccountInfo,
    member: &Member,
    is_mega: bool,
) -> Result<TokenAccount, RegistryError> {
    if is_mega {
        if &member.spt_mega != pool_token_acc_info.key {
            return Err(RegistryErrorCode::InvalidPoolToken)?;
        }
    } else {
        if &member.spt != pool_token_acc_info.key {
            return Err(RegistryErrorCode::InvalidPoolToken)?;
        }
    }

    let token = token_account(pool_token_acc_info)?;
    if &token.mint != pool_mint_acc_info.key {
        return Err(RegistryErrorCode::InvalidPoolTokenMint)?;
    }

    Ok(token)
}

pub fn vault_authenticated(
    acc_info: &AccountInfo,
    vault_authority_acc_info: &AccountInfo,
    registrar_acc_info: &AccountInfo,
    registrar: &Registrar,
    program_id: &Pubkey,
) -> Result<TokenAccount, RegistryError> {
    let v = vault(acc_info, registrar_acc_info, registrar, program_id)?;

    if v.owner != *vault_authority_acc_info.key {
        return Err(RegistryErrorCode::InvalidVaultAuthority)?;
    }

    Ok(v)
}

pub fn vault(
    acc_info: &AccountInfo,
    registrar_acc_info: &AccountInfo,
    registrar: &Registrar,
    program_id: &Pubkey,
) -> Result<TokenAccount, RegistryError> {
    if registrar.vault != *acc_info.key && registrar.mega_vault != *acc_info.key {
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

pub fn vault_init(
    vault_acc_info: &AccountInfo,
    registrar_acc_info: &AccountInfo,
    rent: &Rent,
    nonce: u8,
    program_id: &Pubkey,
) -> Result<(), RegistryError> {
    let vault_authority = Pubkey::create_program_address(
        &vault::signer_seeds(registrar_acc_info.key, &nonce),
        program_id,
    )
    .map_err(|_| RegistryErrorCode::InvalidVaultNonce)?;
    let vault = token(vault_acc_info, &vault_authority)?;
    if vault.owner != vault_authority {
        return Err(RegistryErrorCode::InvalidVaultAuthority)?;
    }
    if !rent.is_exempt(vault_acc_info.lamports(), vault_acc_info.try_data_len()?) {
        return Err(RegistryErrorCode::NotRentExempt)?;
    }
    Ok(())
}

pub fn pending_withdrawal(
    acc_info: &AccountInfo,
    program_id: &Pubkey,
) -> Result<PendingWithdrawal, RegistryError> {
    let pw = PendingWithdrawal::unpack(&acc_info.try_borrow_data()?)?;
    if acc_info.owner != program_id {
        return Err(RegistryErrorCode::InvalidAccountOwner)?;
    }
    if !pw.initialized {
        return Err(RegistryErrorCode::NotInitialized)?;
    }
    if pw.burned {
        return Err(RegistryErrorCode::AlreadyBurned)?;
    }
    Ok(pw)
}

pub fn locked_reward_vendor(
    vendor_acc_info: &AccountInfo,
    registrar_acc_info: &AccountInfo,
    program_id: &Pubkey,
) -> Result<LockedRewardVendor, RegistryError> {
    if vendor_acc_info.owner != program_id {
        return Err(RegistryErrorCode::InvalidAccountOwner)?;
    }
    let vendor = LockedRewardVendor::unpack(&vendor_acc_info.try_borrow_data()?)?;
    if !vendor.initialized {
        return Err(RegistryErrorCode::NotInitialized)?;
    }
    if vendor.registrar != *registrar_acc_info.key {
        return Err(RegistryErrorCode::VendorRegistrarMismatch)?;
    }
    Ok(vendor)
}

pub fn unlocked_reward_vendor(
    vendor_acc_info: &AccountInfo,
    registrar_acc_info: &AccountInfo,
    program_id: &Pubkey,
) -> Result<UnlockedRewardVendor, RegistryError> {
    if vendor_acc_info.owner != program_id {
        return Err(RegistryErrorCode::InvalidAccountOwner)?;
    }
    let vendor = UnlockedRewardVendor::unpack(&vendor_acc_info.try_borrow_data()?)?;
    if !vendor.initialized {
        return Err(RegistryErrorCode::NotInitialized)?;
    }
    if vendor.registrar != *registrar_acc_info.key {
        return Err(RegistryErrorCode::VendorRegistrarMismatch)?;
    }
    Ok(vendor)
}

pub fn token(acc_info: &AccountInfo, authority: &Pubkey) -> Result<TokenAccount, RegistryError> {
    let token = token_account(acc_info)?;
    if token.owner != *authority {
        return Err(RegistryErrorCode::InvalidOwner)?;
    }
    Ok(token)
}

pub fn token_account(acc_info: &AccountInfo) -> Result<TokenAccount, RegistryError> {
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

pub fn clock(acc_info: &AccountInfo) -> Result<Clock, RegistryError> {
    if *acc_info.key != solana_sdk::sysvar::clock::id() {
        return Err(RegistryErrorCode::InvalidClockSysvar)?;
    }
    Clock::from_account_info(acc_info).map_err(Into::into)
}

pub fn mint(acc_info: &AccountInfo) -> Result<Mint, RegistryError> {
    if *acc_info.owner != spl_token::ID {
        return Err(RegistryErrorCode::InvalidAccountOwner)?;
    }

    Mint::unpack(&acc_info.try_borrow_data()?).map_err(Into::into)
}
