use serum_lockup::accounts::vault;
use serum_lockup::accounts::Vesting;
use solana_sdk::account_info::AccountInfo;
use solana_sdk::entrypoint::ProgramResult;
use solana_sdk::instruction::Instruction;
use solana_sdk::pubkey::Pubkey;

pub mod access_control;

// Prepends the program's unique TAG identifier before making the signed
// cross program invocation to a *trusted* program on the whitelist.
//
// The trusted program should perform three steps of validation:
//
// 1. Check for the TAG identifier in the first 8 bytes of the instruction data.
//    If present, then authentication must be done on the following two steps.
// 2. Check accounts[1] is signed.
// 3. Check accounts[1] is the correct program derived address for the vesting
//    account, i.e., signer seeds == [safe_address, beneficiary_address, nonce].
//
// If all of these hold, a program can trust the instruction was invoked
// by a the lockup program on behalf of a vesting account.
//
// Importantly, it's the responsibility of the trusted program to maintain the
// locked invariant preserved by this program and to return the funds at an
// unspecified point in the future for unlocking. Any bug in the trusted program
// can result in locked funds becoming unlocked, so take care when adding to the
// whitelist.
pub fn whitelist_cpi(
    mut instruction: Instruction,
    safe: &Pubkey,
    beneficiary_acc_info: &AccountInfo,
    vesting: &Vesting,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let mut data = serum_lockup::instruction::TAG.to_le_bytes().to_vec();
    data.extend(instruction.data);

    instruction.data = data;

    let signer_seeds = vault::signer_seeds(safe, beneficiary_acc_info.key, &vesting.nonce);
    solana_sdk::program::invoke_signed(&instruction, accounts, &[&signer_seeds])
}
