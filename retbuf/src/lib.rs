use solana_sdk::entrypoint_deprecated;
use solana_sdk::{pubkey::Pubkey, account_info::AccountInfo, entrypoint_deprecated::ProgramResult, program_error::ProgramError};
use std::ops::DerefMut;
use arrayref::{array_refs, mut_array_refs};

fn fast_copy(mut src: &[u8], mut dst: &mut [u8]) {
    loop {
        assert_eq!(src.len(), dst.len());
        if src.len() < 8 {
            break;
        }
        let (src_word, src_rem) = array_refs![src, 8; ..;];
        let (dst_word, dst_rem) = mut_array_refs![dst, 8; ..;];
        *dst_word = *src_word;
        src = src_rem;
        dst = dst_rem;
    }
    assert_eq!(src.len(), dst.len());
    // dst.copy_from_slice(src);
    unsafe {
        std::ptr::copy_nonoverlapping(
            src.as_ptr(),
            dst.as_mut_ptr(),
            src.len(),
        );
    }
}

// fn process_instruction(
//     _program_id: &Pubkey,
//     accounts: &[AccountInfo],
//     instruction_data: &[u8],
// ) -> ProgramResult {
//     let mut dst_ref = accounts
//         .first()
//         .ok_or(ProgramError::NotEnoughAccountKeys)?
//         .try_borrow_mut_data()?;
//     dst_ref.deref_mut()
//         .get_mut(..instruction_data.len())
//         .map(|dst| fast_copy(instruction_data, dst))
//         .ok_or(ProgramError::AccountDataTooSmall)
// }

// entrypoint_deprecated!(process_instruction);

const INVALID_ACCOUNT_DATA: u64 = 4 << 32;
const ACCOUNT_DATA_TOO_SMALL: u64 = 5 << 32;
const NOT_ENOUGH_ACCOUNT_KEYS: u64 = 11 << 32;

#[no_mangle]
pub unsafe extern "C" fn entrypoint(input: *mut u8) -> u64 {
    let num_accounts = std::ptr::read_unaligned(input as *mut u64);
    if num_accounts == 0 {
        return NOT_ENOUGH_ACCOUNT_KEYS;
    } else if num_accounts > 1 {
        return INVALID_ACCOUNT_DATA;;
    }
    
    let data_len = std::ptr::read_unaligned(input.add(43) as *mut u64) as usize;
    let data_ptr = input.add(51);
    let data = std::slice::from_raw_parts_mut(data_ptr, data_len);
    
    let instruction_region_ptr = data_ptr.add(data_len + 41);
    let instruction_data_ptr = data_ptr.add(data_len + 41 + 8);
    let instruction_data_len = std::ptr::read_unaligned(instruction_region_ptr as *mut u64) as usize;
    let instruction_data = std::slice::from_raw_parts(instruction_data_ptr, instruction_data_len);

    match data.get_mut(..instruction_data_len) {
        None => return ACCOUNT_DATA_TOO_SMALL,
        Some(dst) => fast_copy(instruction_data, dst),
    };

    entrypoint_deprecated::SUCCESS
}
