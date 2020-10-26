use solana_program::{
    pubkey::Pubkey,
    account_info::AccountInfo,
    program_error::ProgramError,
    entrypoint::ProgramResult,
    info,
    entrypoint,
};

entrypoint!(entry);
fn entry(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    let account = accounts.get(0).ok_or(ProgramError::NotEnoughAccountKeys)?;
    if instruction_data.len() != 32 {
        return Err(ProgramError::InvalidInstructionData);
    }
    let expected_owner = Pubkey::new(instruction_data);
    if expected_owner != *account.owner {
        info!("Account owner mismatch");
        return Err(ProgramError::Custom(0x100));
    }
    Ok(())
}
