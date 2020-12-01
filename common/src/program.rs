use solana_sdk::account_info::AccountInfo;
use solana_sdk::program_error::ProgramError;
use spl_token::instruction as token_instruction;
use std::convert::Into;

pub fn invoke_token_transfer<'a, 'b>(
    from_acc_info: &'a AccountInfo<'b>,
    to_acc_info: &'a AccountInfo<'b>,
    authority_acc_info: &'a AccountInfo<'b>,
    tok_program_acc_info: &'a AccountInfo<'b>,
    signer_seeds: &[&[&[u8]]],
    amount: u64,
) -> Result<(), ProgramError> {
    let transfer_instr = token_instruction::transfer(
        &spl_token::ID,
        from_acc_info.key,
        to_acc_info.key,
        authority_acc_info.key,
        &[],
        amount,
    )?;
    solana_sdk::program::invoke_signed(
        &transfer_instr,
        &[
            from_acc_info.clone(),
            to_acc_info.clone(),
            authority_acc_info.clone(),
            tok_program_acc_info.clone(),
        ],
        signer_seeds,
    )
}
