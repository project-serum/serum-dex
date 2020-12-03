use serum_common::pack::Pack;
use serum_registry::access_control;
use serum_registry::accounts::{vault, Member, MemberBalances, Registrar};
use serum_registry::error::{RegistryError, RegistryErrorCode};
use solana_program::info;
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::program_option::COption;
use solana_sdk::program_pack::Pack as TokenPack;
use solana_sdk::pubkey::Pubkey;
use spl_token::instruction as token_instruction;
use spl_token::state::Account as TokenAccount;

#[inline(never)]
pub fn handler(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    delegate: Pubkey,
) -> Result<(), RegistryError> {
    info!("handler: create_member");

    let acc_infos = &mut accounts.iter();

    let beneficiary_acc_info = next_account_info(acc_infos)?;
    let member_acc_info = next_account_info(acc_infos)?;
    let entity_acc_info = next_account_info(acc_infos)?;
    let registrar_acc_info = next_account_info(acc_infos)?;
    let registry_signer_acc_info = next_account_info(acc_infos)?;
    let spt_acc_info = next_account_info(acc_infos)?;
    let spt_mega_acc_info = next_account_info(acc_infos)?;
    let token_program_acc_info = next_account_info(acc_infos)?;
    let rent_acc_info = next_account_info(acc_infos)?;

    let AccessControlResponse { registrar } = access_control(AccessControlRequest {
        beneficiary_acc_info,
        member_acc_info,
        entity_acc_info,
        registrar_acc_info,
        registry_signer_acc_info,
        rent_acc_info,
        program_id,
        spt_acc_info,
        spt_mega_acc_info,
    })?;

    Member::unpack_unchecked_mut(
        &mut member_acc_info.try_borrow_mut_data()?,
        &mut |member: &mut Member| {
            state_transition(StateTransitionRequest {
                beneficiary_acc_info,
                member,
                delegate,
                entity_acc_info,
                registrar_acc_info,
                registrar: &registrar,
                registry_signer_acc_info,
                spt_acc_info,
                spt_mega_acc_info,
                token_program_acc_info,
            })
            .map_err(Into::into)
        },
    )?;

    Ok(())
}

#[inline(never)]
fn access_control(req: AccessControlRequest) -> Result<AccessControlResponse, RegistryError> {
    info!("access-control: create_member");

    let AccessControlRequest {
        beneficiary_acc_info,
        member_acc_info,
        entity_acc_info,
        rent_acc_info,
        registrar_acc_info,
        registry_signer_acc_info,
        program_id,
        spt_acc_info,
        spt_mega_acc_info,
    } = req;

    // Authorization.
    if !beneficiary_acc_info.is_signer {
        return Err(RegistryErrorCode::Unauthorized)?;
    }

    // Account validation.
    let rent = access_control::rent(rent_acc_info)?;
    let registrar = access_control::registrar(registrar_acc_info, program_id)?;
    let _ = access_control::entity(entity_acc_info, registrar_acc_info, program_id)?;

    // CreateMember specific.
    if !rent.is_exempt(member_acc_info.lamports(), member_acc_info.try_data_len()?) {
        return Err(RegistryErrorCode::NotRentExempt)?;
    }
    // Use unpack_unchecked since the data will be zero initialized
    // and so won't consume the entire slice (since Member has internal
    // state using Vecs).
    let mut data: &[u8] = &member_acc_info.try_borrow_data()?;
    let member = Member::unpack_unchecked(&mut data)?;
    if member_acc_info.owner != program_id {
        return Err(RegistryErrorCode::InvalidAccountOwner)?;
    }
    if member.initialized {
        return Err(RegistryErrorCode::AlreadyInitialized)?;
    }

    // Pool token owner must be the program derived address.
    // Delegate must be None; it will be set in this instruction.
    let spt = TokenAccount::unpack(&spt_acc_info.try_borrow_data()?)?;
    if spt.delegate != COption::None {
        return Err(RegistryErrorCode::SptDelegateAlreadySet)?;
    }
    if spt.owner != *registry_signer_acc_info.key {
        return Err(RegistryErrorCode::InvalidStakeTokenOwner)?;
    }
    if spt.mint != registrar.pool_mint {
        return Err(RegistryErrorCode::InvalidMint)?;
    }
    let spt_mega = TokenAccount::unpack(&spt_mega_acc_info.try_borrow_data()?)?;
    if spt_mega.delegate != COption::None {
        return Err(RegistryErrorCode::SptDelegateAlreadySet)?;
    }
    if spt_mega.owner != *registry_signer_acc_info.key {
        return Err(RegistryErrorCode::InvalidStakeTokenOwner)?;
    }
    if spt_mega.mint != registrar.pool_mint_mega {
        return Err(RegistryErrorCode::InvalidMint)?;
    }

    Ok(AccessControlResponse { registrar })
}

fn state_transition(req: StateTransitionRequest) -> Result<(), RegistryError> {
    info!("state-transition: create_member");

    let StateTransitionRequest {
        beneficiary_acc_info,
        member,
        delegate,
        entity_acc_info,
        registrar_acc_info,
        registrar,
        registry_signer_acc_info,
        spt_acc_info,
        spt_mega_acc_info,
        token_program_acc_info,
    } = req;

    approve_delegate(
        beneficiary_acc_info,
        token_program_acc_info,
        registrar_acc_info,
        registrar,
        registry_signer_acc_info,
        spt_acc_info,
    )?;

    approve_delegate(
        beneficiary_acc_info,
        token_program_acc_info,
        registrar_acc_info,
        registrar,
        registry_signer_acc_info,
        spt_mega_acc_info,
    )?;

    member.initialized = true;
    member.registrar = *registrar_acc_info.key;
    member.entity = *entity_acc_info.key;
    member.beneficiary = *beneficiary_acc_info.key;
    member.balances = MemberBalances::new(*beneficiary_acc_info.key, delegate);
    member.spt = *spt_acc_info.key;
    member.spt_mega = *spt_mega_acc_info.key;

    Ok(())
}

#[inline(always)]
fn approve_delegate<'a, 'b, 'c>(
    beneficiary_acc_info: &'a AccountInfo<'b>,
    token_program_acc_info: &'a AccountInfo<'b>,
    registrar_acc_info: &'a AccountInfo<'b>,
    registrar: &'c Registrar,
    registry_signer_acc_info: &'a AccountInfo<'b>,
    spt_acc_info: &'a AccountInfo<'b>,
) -> Result<(), RegistryError> {
    let approve_instr = token_instruction::approve(
        &spl_token::ID,
        spt_acc_info.key,
        &beneficiary_acc_info.key,
        registry_signer_acc_info.key,
        &[],
        0,
    )?;
    solana_sdk::program::invoke_signed(
        &approve_instr,
        &[
            spt_acc_info.clone(),
            beneficiary_acc_info.clone(),
            registry_signer_acc_info.clone(),
            token_program_acc_info.clone(),
        ],
        &[&vault::signer_seeds(
            registrar_acc_info.key,
            &registrar.nonce,
        )],
    )?;

    Ok(())
}

struct AccessControlRequest<'a, 'b> {
    beneficiary_acc_info: &'a AccountInfo<'b>,
    member_acc_info: &'a AccountInfo<'b>,
    entity_acc_info: &'a AccountInfo<'b>,
    registrar_acc_info: &'a AccountInfo<'b>,
    registry_signer_acc_info: &'a AccountInfo<'b>,
    rent_acc_info: &'a AccountInfo<'b>,
    spt_acc_info: &'a AccountInfo<'b>,
    spt_mega_acc_info: &'a AccountInfo<'b>,
    program_id: &'a Pubkey,
}

struct AccessControlResponse {
    registrar: Registrar,
}

struct StateTransitionRequest<'a, 'b, 'c> {
    beneficiary_acc_info: &'a AccountInfo<'b>,
    registrar_acc_info: &'a AccountInfo<'b>,
    entity_acc_info: &'a AccountInfo<'b>,
    spt_acc_info: &'a AccountInfo<'b>,
    spt_mega_acc_info: &'a AccountInfo<'b>,
    registry_signer_acc_info: &'a AccountInfo<'b>,
    token_program_acc_info: &'a AccountInfo<'b>,
    registrar: &'c Registrar,
    member: &'c mut Member,
    delegate: Pubkey,
}
