use crate::accounts::reward_queue::Ring;
use crate::accounts::{
    vault, BalanceSandbox, Entity, LockedRewardVendor, Member, PendingWithdrawal, Registrar,
    RewardEventQueue, UnlockedRewardVendor,
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
use solana_sdk::program_option::COption;
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

#[inline(never)]
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

#[inline(never)]
pub fn member_vault(
    member: &Member,
    member_vault_acc_info: &AccountInfo,
    member_vault_authority_acc_info: &AccountInfo,
    registrar_acc_info: &AccountInfo,
    registrar: &Registrar,
    program_id: &Pubkey,
    balance_id: &Pubkey,
) -> Result<(TokenAccount, bool), RegistryError> {
    let member_vault = vault_authenticated(
        member_vault_acc_info,
        member_vault_authority_acc_info,
        registrar_acc_info,
        &registrar,
        program_id,
    )?;
    let b = member
        .balances
        .iter()
        .filter(|b| &b.owner == balance_id)
        .collect::<Vec<&BalanceSandbox>>();
    let balances = b.first().ok_or(RegistryErrorCode::InvalidBalanceSandbox)?;

    let is_mega = {
        if &balances.vault != member_vault_acc_info.key
            && &balances.vault_mega != member_vault_acc_info.key
        {
            return Err(RegistryErrorCode::InvalidVault)?;
        }
        member_vault_acc_info.key == &balances.vault_mega
    };

    Ok((member_vault, is_mega))
}

#[inline(never)]
pub fn member_vault_stake(
    member: &Member,
    member_vault_acc_info: &AccountInfo,
    member_vault_authority_acc_info: &AccountInfo,
    registrar_acc_info: &AccountInfo,
    registrar: &Registrar,
    program_id: &Pubkey,
    balance_id: &Pubkey,
) -> Result<(TokenAccount, bool), RegistryError> {
    let member_vault = vault_authenticated(
        member_vault_acc_info,
        member_vault_authority_acc_info,
        registrar_acc_info,
        &registrar,
        program_id,
    )?;

    let b = member
        .balances
        .iter()
        .filter(|b| &b.owner == balance_id)
        .collect::<Vec<&BalanceSandbox>>();
    let balances = b.first().ok_or(RegistryErrorCode::InvalidBalanceSandbox)?;

    let is_mega = {
        if member_vault_acc_info.key != &balances.vault_stake
            && member_vault_acc_info.key != &balances.vault_stake_mega
        {
            return Err(RegistryErrorCode::InvalidStakeVault)?;
        }
        member_vault_acc_info.key == &balances.vault_stake_mega
    };

    Ok((member_vault, is_mega))
}

pub fn member_vault_pending_withdrawal(
    member: &Member,
    member_vault_acc_info: &AccountInfo,
    member_vault_authority_acc_info: &AccountInfo,
    registrar_acc_info: &AccountInfo,
    registrar: &Registrar,
    program_id: &Pubkey,
    balance_id: &Pubkey,
) -> Result<(TokenAccount, bool), RegistryError> {
    let member_vault = vault_authenticated(
        member_vault_acc_info,
        member_vault_authority_acc_info,
        registrar_acc_info,
        &registrar,
        program_id,
    )?;

    let b = member
        .balances
        .iter()
        .filter(|b| &b.owner == balance_id)
        .collect::<Vec<&BalanceSandbox>>();
    let balances = b.first().ok_or(RegistryErrorCode::InvalidBalanceSandbox)?;

    let is_mega = {
        if member_vault_acc_info.key != &balances.vault_pending_withdrawal
            && member_vault_acc_info.key != &balances.vault_pending_withdrawal_mega
        {
            return Err(RegistryErrorCode::InvalidPendingWithdrawalVault)?;
        }
        member_vault_acc_info.key == &balances.vault_pending_withdrawal_mega
    };

    Ok((member_vault, is_mega))
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

pub fn member_pool_token(
    member: &Member,
    pool_token_acc_info: &AccountInfo,
    pool_mint_acc_info: &AccountInfo,
    balance_id: &Pubkey,
    is_mega: bool,
) -> Result<TokenAccount, RegistryError> {
    let b = member
        .balances
        .iter()
        .filter(|b| &b.owner == balance_id)
        .collect::<Vec<&BalanceSandbox>>();
    let balances = b.first().ok_or(RegistryErrorCode::InvalidBalanceSandbox)?;

    if is_mega {
        if &balances.spt_mega != pool_token_acc_info.key {
            return Err(RegistryErrorCode::InvalidPoolToken)?;
        }
    } else {
        if &balances.spt != pool_token_acc_info.key {
            return Err(RegistryErrorCode::InvalidPoolToken)?;
        }
    }

    let token = token_account(pool_token_acc_info)?;
    if &token.mint != pool_mint_acc_info.key {
        return Err(RegistryErrorCode::InvalidPoolToken)?;
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

fn vault(
    acc_info: &AccountInfo,
    registrar_acc_info: &AccountInfo,
    registrar: &Registrar,
    program_id: &Pubkey,
) -> Result<TokenAccount, RegistryError> {
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

pub fn vault_pair(
    vault_acc_info: &AccountInfo,
    vault_mega_acc_info: &AccountInfo,
    registrar_acc_info: &AccountInfo,
    registrar: &Registrar,
    rent: &Rent,
    program_id: &Pubkey,
) -> Result<(), RegistryError> {
    let v = vault_init(
        vault_acc_info,
        registrar_acc_info,
        rent,
        registrar.nonce,
        program_id,
    )?;
    let v_mega = vault_init(
        vault_mega_acc_info,
        registrar_acc_info,
        rent,
        registrar.nonce,
        program_id,
    )?;

    if v.mint != registrar.mint {
        return Err(RegistryErrorCode::InvalidMint)?;
    }
    if v_mega.mint != registrar.mega_mint {
        return Err(RegistryErrorCode::InvalidMint)?;
    }
    Ok(())
}

pub fn pool_token_pair(
    spt_acc_info: &AccountInfo,
    spt_mega_acc_info: &AccountInfo,
    registrar: &Registrar,
    registry_signer_acc_info: &AccountInfo,
) -> Result<(), RegistryError> {
    let spt = TokenAccount::unpack(&spt_acc_info.try_borrow_data()?)?;
    let spt_mega = TokenAccount::unpack(&spt_mega_acc_info.try_borrow_data()?)?;
    // Pool token owner must be the program derived address.
    // Delegate must be None, since it will be set in this instruction.
    if spt.delegate != COption::None {
        return Err(RegistryErrorCode::SptDelegateAlreadySet)?;
    }
    if spt.owner != *registry_signer_acc_info.key {
        return Err(RegistryErrorCode::InvalidStakeTokenOwner)?;
    }
    if spt.mint != registrar.pool_mint {
        return Err(RegistryErrorCode::InvalidMint)?;
    }
    if spt_mega.delegate != COption::None {
        return Err(RegistryErrorCode::SptDelegateAlreadySet)?;
    }
    if spt_mega.owner != *registry_signer_acc_info.key {
        return Err(RegistryErrorCode::InvalidStakeTokenOwner)?;
    }
    if spt_mega.mint != registrar.pool_mint_mega {
        return Err(RegistryErrorCode::InvalidMint)?;
    }
    Ok(())
}

pub fn vault_init(
    vault_acc_info: &AccountInfo,
    registrar_acc_info: &AccountInfo,
    rent: &Rent,
    nonce: u8,
    program_id: &Pubkey,
) -> Result<TokenAccount, RegistryError> {
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
    Ok(vault)
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

pub fn balance_sandbox(
    balances: &[BalanceSandboxAccInfo],
    registrar_acc_info: &AccountInfo,
    registrar: &Registrar,
    registry_signer_acc_info: &AccountInfo,
    rent: &Rent,
    program_id: &Pubkey,
) -> Result<(), RegistryError> {
    // Only allow two sets of balances for now (main and locked).
    if balances.len() != 2 {
        return Err(RegistryErrorCode::InvalidAssetsLen)?;
    }
    // Check the given balance assets are all unique accounts.
    let mut all_accounts = vec![];
    for b in balances {
        // Pool Tokens.
        pool_token_pair(
            b.spt_acc_info,
            b.spt_mega_acc_info,
            registrar,
            registry_signer_acc_info,
        )?;

        // Deposits.
        vault_pair(
            b.vault_acc_info,
            b.vault_mega_acc_info,
            registrar_acc_info,
            &registrar,
            &rent,
            program_id,
        )?;
        // Stake.
        vault_pair(
            b.vault_stake_acc_info,
            b.vault_stake_mega_acc_info,
            registrar_acc_info,
            &registrar,
            &rent,
            program_id,
        )?;
        // Pending withdrawals.
        vault_pair(
            b.vault_pw_acc_info,
            b.vault_pw_mega_acc_info,
            registrar_acc_info,
            &registrar,
            &rent,
            program_id,
        )?;

        all_accounts.extend_from_slice(&[
            b.owner_acc_info.key,
            b.spt_acc_info.key,
            b.spt_mega_acc_info.key,
            b.vault_acc_info.key,
            b.vault_mega_acc_info.key,
            b.vault_stake_acc_info.key,
            b.vault_stake_mega_acc_info.key,
            b.vault_pw_acc_info.key,
            b.vault_pw_mega_acc_info.key,
        ]);
    }
    let given_len = all_accounts.len();
    all_accounts.sort();
    all_accounts.dedup();
    if given_len != all_accounts.len() {
        return Err(RegistryErrorCode::InvalidAssetsLen)?;
    }
    Ok(())
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

pub struct BalanceSandboxAccInfo<'a, 'b> {
    pub owner_acc_info: &'a AccountInfo<'b>,
    pub spt_acc_info: &'a AccountInfo<'b>,
    pub spt_mega_acc_info: &'a AccountInfo<'b>,
    pub vault_acc_info: &'a AccountInfo<'b>,
    pub vault_mega_acc_info: &'a AccountInfo<'b>,
    pub vault_stake_acc_info: &'a AccountInfo<'b>,
    pub vault_stake_mega_acc_info: &'a AccountInfo<'b>,
    pub vault_pw_acc_info: &'a AccountInfo<'b>,
    pub vault_pw_mega_acc_info: &'a AccountInfo<'b>,
}
