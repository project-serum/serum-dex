//! Instruction types

#![allow(clippy::too_many_arguments)]

use crate::error::EtfError;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    program_error::ProgramError,
    pubkey::Pubkey,
};
use std::convert::TryInto;
use std::mem::size_of;

/// Instructions supported by the EtfInfo program.
#[repr(C)]
#[derive(Clone, Debug, PartialEq)]
pub enum EtfInstruction {
    ///   Initializes a new EtfInfo.
    ///
    ///   0. `[writable, signer]` New Token-etf to create.
    ///   1. `[]` $authority derived from `create_program_address(&[Token-etf account])`
    ///   2. `[]` token_a Account. Must be non zero, owned by $authority.
    ///   3. `[]` token_b Account. Must be non zero, owned by $authority.
    ///   4. `[writable]` pool Token. Must be empty, owned by $authority.
    ///   5. `[writable]` Pool Account to deposit the generated tokens, user is the owner.
    ///   6. '[]` Token program id
    Initialize {
        /// nonce used to create valid program address
        nonce: u8,
    },

    ///   Deposit some tokens into the pool.  The output is a "pool" token representing ownership
    ///   into the pool. Inputs are converted to the current ratio.
    ///
    ///   0. `[]` Token-etf
    ///   1. `[]` $authority
    ///   2. `[writable]` token_a $authority can transfer amount,
    ///   3. `[writable]` token_b $authority can transfer amount,
    ///   4. `[writable]` token_a Base Account to deposit into.
    ///   5. `[writable]` token_b Base Account to deposit into.
    ///   6. `[writable]` Pool MINT account, $authority is the owner.
    ///   7. `[writable]` Pool Account to deposit the generated tokens, user is the owner.
    ///   8. '[]` Token program id
    Deposit {
        /// Pool token amount to transfer. token_a and token_b amount are set by
        /// the current exchange rate and size of the pool
        amount: u64,
    },

    ///   Withdraw the token from the pool at the current ratio.
    ///
    ///   0. `[]` Token-etf
    ///   1. `[]` $authority
    ///   2. `[writable]` Pool mint account, $authority is the owner
    ///   3. `[writable]` SOURCE Pool account, amount is transferable by $authority.
    ///   4. `[writable]` token_a Etf Account to withdraw FROM.
    ///   5. `[writable]` token_b Etf Account to withdraw FROM.
    ///   6. `[writable]` token_a user Account to credit.
    ///   7. `[writable]` token_b user Account to credit.
    ///   8. '[]` Token program id
    Withdraw {
        /// Amount of pool tokens to burn. User receives an output of token a
        /// and b based on the percentage of the pool tokens that are returned.
        amount: u64,
    },
}

impl EtfInstruction {
    /// Unpacks a byte buffer into a [EtfInstruction](enum.EtfInstruction.html).
    pub fn unpack(input: &[u8]) -> Result<Self, ProgramError> {
        let (&tag, rest) = input.split_first().ok_or(EtfError::InvalidInstruction)?;
        Ok(match tag {
            0 => {
                let (&nonce, _rest) = rest.split_first().ok_or(EtfError::InvalidInstruction)?;
                Self::Initialize {
                    nonce,
                }
            }
            1 | 2  => {
                let (amount, _rest) = Self::unpack_u64(rest)?;
                match tag {
                    1 => Self::Deposit { amount },
                    2 => Self::Withdraw { amount },
                    _ => unreachable!(),
                }
            }
            _ => return Err(EtfError::InvalidInstruction.into()),
        })
    }

    fn unpack_u64(input: &[u8]) -> Result<(u64, &[u8]), ProgramError> {
        if input.len() >= 8 {
            let (amount, rest) = input.split_at(8);
            let amount = amount
                .get(..8)
                .and_then(|slice| slice.try_into().ok())
                .map(u64::from_le_bytes)
                .ok_or(EtfError::InvalidInstruction)?;
            Ok((amount, rest))
        } else {
            Err(EtfError::InvalidInstruction.into())
        }
    }

    /// Packs a [EtfInstruction](enum.EtfInstruction.html) into a byte buffer.
    pub fn pack(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(size_of::<Self>());
        match *self {
            Self::Initialize {
                nonce,
            } => {
                buf.push(0);
                buf.push(nonce);
            }
            Self::Deposit { amount } => {
                buf.push(1);
                buf.extend_from_slice(&amount.to_le_bytes());
            }
            Self::Withdraw { amount } => {
                buf.push(2);
                buf.extend_from_slice(&amount.to_le_bytes());
            }
        }
        buf
    }
}

/// Creates an 'initialize' instruction.
pub fn initialize(
    program_id: &Pubkey,
    token_program_id: &Pubkey,
    etf_pubkey: &Pubkey,
    authority_pubkey: &Pubkey,
    token_a_pubkey: &Pubkey,
    token_b_pubkey: &Pubkey,
    pool_pubkey: &Pubkey,
    destination_pubkey: &Pubkey,
    nonce: u8,
) -> Result<Instruction, ProgramError> {
    let init_data = EtfInstruction::Initialize {
        nonce,
    };
    let data = init_data.pack();

    let accounts = vec![
        AccountMeta::new(*etf_pubkey, true),
        AccountMeta::new(*authority_pubkey, false),
        AccountMeta::new(*token_a_pubkey, false),
        AccountMeta::new(*token_b_pubkey, false),
        AccountMeta::new(*pool_pubkey, false),
        AccountMeta::new(*destination_pubkey, false),
        AccountMeta::new(*token_program_id, false),
    ];

    Ok(Instruction {
        program_id: *program_id,
        accounts,
        data,
    })
}

/// Creates a 'deposit' instruction.
pub fn deposit(
    program_id: &Pubkey,
    token_program_id: &Pubkey,
    etf_pubkey: &Pubkey,
    authority_pubkey: &Pubkey,
    deposit_token_a_pubkey: &Pubkey,
    deposit_token_b_pubkey: &Pubkey,
    etf_token_a_pubkey: &Pubkey,
    etf_token_b_pubkey: &Pubkey,
    pool_mint_pubkey: &Pubkey,
    destination_pubkey: &Pubkey,
    amount: u64,
) -> Result<Instruction, ProgramError> {
    let data = EtfInstruction::Deposit { amount }.pack();

    let accounts = vec![
        AccountMeta::new(*etf_pubkey, false),
        AccountMeta::new(*authority_pubkey, false),
        AccountMeta::new(*deposit_token_a_pubkey, false),
        AccountMeta::new(*deposit_token_b_pubkey, false),
        AccountMeta::new(*etf_token_a_pubkey, false),
        AccountMeta::new(*etf_token_b_pubkey, false),
        AccountMeta::new(*pool_mint_pubkey, false),
        AccountMeta::new(*destination_pubkey, false),
        AccountMeta::new(*token_program_id, false),
    ];

    Ok(Instruction {
        program_id: *program_id,
        accounts,
        data,
    })
}

/// Creates a 'withdraw' instruction.
pub fn withdraw(
    program_id: &Pubkey,
    token_program_id: &Pubkey,
    etf_pubkey: &Pubkey,
    authority_pubkey: &Pubkey,
    pool_mint_pubkey: &Pubkey,
    source_pubkey: &Pubkey,
    etf_token_a_pubkey: &Pubkey,
    etf_token_b_pubkey: &Pubkey,
    destination_token_a_pubkey: &Pubkey,
    destination_token_b_pubkey: &Pubkey,
    amount: u64,
) -> Result<Instruction, ProgramError> {
    let data = EtfInstruction::Withdraw { amount }.pack();

    let accounts = vec![
        AccountMeta::new(*etf_pubkey, false),
        AccountMeta::new(*authority_pubkey, false),
        AccountMeta::new(*pool_mint_pubkey, false),
        AccountMeta::new(*source_pubkey, false),
        AccountMeta::new(*etf_token_a_pubkey, false),
        AccountMeta::new(*etf_token_b_pubkey, false),
        AccountMeta::new(*destination_token_a_pubkey, false),
        AccountMeta::new(*destination_token_b_pubkey, false),
        AccountMeta::new(*token_program_id, false),
    ];

    Ok(Instruction {
        program_id: *program_id,
        accounts,
        data,
    })
}

/// Unpacks a reference from a bytes buffer.
/// TODO actually pack / unpack instead of relying on normal memory layout.
pub fn unpack<T>(input: &[u8]) -> Result<&T, ProgramError> {
    if input.len() < size_of::<u8>() + size_of::<T>() {
        return Err(ProgramError::InvalidAccountData);
    }
    #[allow(clippy::cast_ptr_alignment)]
    let val: &T = unsafe { &*(&input[1] as *const u8 as *const T) };
    Ok(val)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_instruction_packing() {
        let nonce: u8 = 255;
        let check = EtfInstruction::Initialize {
            nonce,
        };
        let packed = check.pack();
        let mut expect = vec![];
        expect.push(0 as u8);
        expect.push(nonce);
        assert_eq!(packed, expect);
        let unpacked = EtfInstruction::unpack(&expect).unwrap();
        assert_eq!(unpacked, check);

        let amount = 5;
        let check = EtfInstruction::Deposit { amount };
        let packed = check.pack();
        let mut expect = vec![1, 5];
        expect.extend_from_slice(&[0u8; 7]);
        assert_eq!(packed, expect);
        let unpacked = EtfInstruction::unpack(&expect).unwrap();
        assert_eq!(unpacked, check);

        let amount: u64 = 1212438012089;
        let check = EtfInstruction::Withdraw { amount };
        let packed = check.pack();
        let mut expect = vec![2];
        expect.extend_from_slice(&amount.to_le_bytes());
        assert_eq!(packed, expect);
        let unpacked = EtfInstruction::unpack(&expect).unwrap();
        assert_eq!(unpacked, check);
    }
}
