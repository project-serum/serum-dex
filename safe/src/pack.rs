// TODO: move this to common.

use arrayref::{array_ref, mut_array_refs};
use solana_client_gen::solana_sdk::program_error::ProgramError;
use spl_token::error::TokenError;
use spl_token::pack::{IsInitialized, Sealed};

/// DynPack is a version of sol_token's Pack trait, but with a dynamic length
/// data array. It's expected the first 8 bytes defines the size of the
/// entire data array.
pub trait DynPack: Sealed {
    #[doc(hidden)]
    fn pack_into_slice(&self, dst: &mut [u8]);
    #[doc(hidden)]
    fn unpack_from_slice(src: &[u8]) -> Result<Self, ProgramError>;
    /// Get the packed length
    fn get_packed_len(&self) -> usize;

    /// Unpack from slice and check if initialized
    fn unpack(input: &[u8]) -> Result<Self, ProgramError>
    where
        Self: IsInitialized,
    {
        let value = Self::unpack_unchecked(input)?;
        if value.is_initialized() {
            Ok(value)
        } else {
            Err(TokenError::UninitializedState.into())
        }
    }

    /// Unpack from slice without checking if initialized
    fn unpack_unchecked(input: &[u8]) -> Result<Self, ProgramError> {
        if input.len() < 8 {
            return Err(ProgramError::InvalidAccountData);
        }
        let size_bytes_input = array_ref![input, 0, 8];
        let size_input = u64::from_le_bytes(*size_bytes_input);

        if input.len() as u64 != size_input {
            return Err(ProgramError::InvalidAccountData);
        }

        Ok(Self::unpack_from_slice(&input[8..])?)
    }

    /// Borrow `Self` from `input` for the duration of the call to `f`, but first check that `Self`
    /// is initialized
    #[inline(never)]
    fn unpack_mut<F, U>(input: &mut [u8], f: &mut F) -> Result<U, ProgramError>
    where
        F: FnMut(&mut Self) -> Result<U, ProgramError>,
        Self: IsInitialized,
    {
        let mut t = Self::unpack(input)?;
        let u = f(&mut t)?;
        Self::pack(t, input)?;
        Ok(u)
    }

    /// Borrow `Self` from `input` for the duration of the call to `f`, without checking that
    /// `Self` has been initialized
    #[inline(never)]
    fn unpack_unchecked_mut<F, U>(input: &mut [u8], f: &mut F) -> Result<U, ProgramError>
    where
        F: FnMut(&mut Self) -> Result<U, ProgramError>,
    {
        let mut t = Self::unpack_unchecked(input)?;
        let u = f(&mut t)?;
        Self::pack(t, input)?;
        Ok(u)
    }

    /// Pack into slice
    fn pack(src: Self, dst: &mut [u8]) -> Result<(), ProgramError> {
        if dst.len() != src.get_packed_len() {
            return Err(ProgramError::InvalidAccountData);
        }
        let (len_dst, data_dst) = mut_array_refs![dst, 8; .. ;];

        len_dst.copy_from_slice(&src.get_packed_len().to_le_bytes());
        src.pack_into_slice(data_dst);

        Ok(())
    }
}
