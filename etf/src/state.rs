//! State transition types

use crate::error::EtfError;
use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use solana_sdk::{program_error::ProgramError, pubkey::Pubkey};

/// Program states.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EtfInfo {
    /// Initialized state.
    pub is_initialized: bool,
    /// Nonce used in program address.
    /// The program address is created deterministically with the nonce,
    /// etf program id, and etf account pubkey.  This program address has
    /// authority over the etf's token A account, token B account, and pool
    /// token mint.
    pub nonce: u8,
    /// Token A
    /// The Liquidity token is issued against this value.
    pub token_a: Pubkey,
    /// Token B
    pub token_b: Pubkey,
    /// Pool tokens are issued when A or B tokens are deposited.
    /// Pool tokens can be withdrawn back to the original A or B token.
    pub pool_mint: Pubkey,
}

impl EtfInfo {
    /// Helper function to get the more efficient packed size of the struct
    const fn get_packed_len() -> usize {
        98
    }

    /// Unpacks a byte buffer into a [EtfInfo](struct.EtfInfo.html) and checks
    /// that it is initialized.
    pub fn unpack(input: &[u8]) -> Result<Self, ProgramError> {
        let value = Self::unpack_unchecked(input)?;
        if value.is_initialized {
            Ok(value)
        } else {
            Err(EtfError::InvalidState.into())
        }
    }

    /// Unpacks a byte buffer into a [EtfInfo](struct.EtfInfo.html).
    pub fn unpack_unchecked(input: &[u8]) -> Result<Self, ProgramError> {
        let input = array_ref![input, 0, EtfInfo::get_packed_len()];
        #[allow(clippy::ptr_offset_with_cast)]
        let (is_initialized, nonce, token_a, token_b, pool_mint) =
            array_refs![input, 1, 1, 32, 32, 32];
        Ok(Self {
            is_initialized: match is_initialized {
                [0] => false,
                [1] => true,
                _ => return Err(ProgramError::InvalidAccountData),
            },
            nonce: nonce[0],
            token_a: Pubkey::new_from_array(*token_a),
            token_b: Pubkey::new_from_array(*token_b),
            pool_mint: Pubkey::new_from_array(*pool_mint),
        })
    }

    /// Packs [EtfInfo](struct.EtfInfo.html) into a byte buffer.
    pub fn pack(&self, output: &mut [u8]) {
        let output = array_mut_ref![output, 0, EtfInfo::get_packed_len()];
        let (is_initialized, nonce, token_a, token_b, pool_mint) =
            mut_array_refs![output, 1, 1, 32, 32, 32];
        is_initialized[0] = self.is_initialized as u8;
        nonce[0] = self.nonce;
        token_a.copy_from_slice(self.token_a.as_ref());
        token_b.copy_from_slice(self.token_b.as_ref());
        pool_mint.copy_from_slice(self.pool_mint.as_ref());
    }
}

/// The Uniswap invariant calculator.
pub struct Invariant {
    /// Token A
    pub token_a: u64,
    /// Token B
    pub token_b: u64,
}

impl Invariant {
    /// Exchange rate
    pub fn exchange_rate(&self, token_a: u64) -> Option<u64> {
        token_a.checked_mul(self.token_b)?.checked_div(self.token_a)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_etf_info_packing() {
        let nonce = 255;
        let token_a_raw = [1u8; 32];
        let token_b_raw = [2u8; 32];
        let pool_mint_raw = [3u8; 32];
        let token_a = Pubkey::new_from_array(token_a_raw);
        let token_b = Pubkey::new_from_array(token_b_raw);
        let pool_mint = Pubkey::new_from_array(pool_mint_raw);
        let is_initialized = true;
        let etf_info = EtfInfo {
            is_initialized,
            nonce,
            token_a,
            token_b,
            pool_mint,
        };

        let mut packed = [0u8; EtfInfo::get_packed_len()];
        etf_info.pack(&mut packed);
        let unpacked = EtfInfo::unpack(&packed).unwrap();
        assert_eq!(etf_info, unpacked);

        let mut packed = vec![];
        packed.push(1 as u8);
        packed.push(nonce);
        packed.extend_from_slice(&token_a_raw);
        packed.extend_from_slice(&token_b_raw);
        packed.extend_from_slice(&pool_mint_raw);
        let unpacked = EtfInfo::unpack(&packed).unwrap();
        assert_eq!(etf_info, unpacked);

        let packed = [0u8; EtfInfo::get_packed_len()];
        let etf_info: EtfInfo = Default::default();
        let unpack_unchecked = EtfInfo::unpack_unchecked(&packed).unwrap();
        assert_eq!(unpack_unchecked, etf_info);
        let err = EtfInfo::unpack(&packed).unwrap_err();
        assert_eq!(err, EtfError::InvalidState.into());
    }
}
