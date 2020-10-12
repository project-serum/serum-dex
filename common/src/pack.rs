//! pack.rs defines utilities for serializing Solana accounts to/from bytes.

use borsh::{BorshDeserialize, BorshSerialize};

// Re-export for users of the `packable` macro.
pub use solana_sdk::program_error::ProgramError;

/// The Pack trait defines Account serialization for Solana programs.
///
/// If possible, don't use `*_unchecked` methods.
pub trait Pack: std::marker::Sized {
    /// Serializes `src` into `dst`. The size of the serialization and
    /// dst must be equal.
    fn pack(src: Self, dst: &mut [u8]) -> Result<(), ProgramError>;

    /// Deserializes `src` into Self. The deserialized object need not
    /// use all the bytes in `src` and should mutated the slice so that
    /// it's new len() becomes the size of the bytes *not* deserialized.
    ///
    /// This is the case, for example, when encoding variable length data,
    /// say, when one has a src array of all zeroes, and a Self that has a
    /// Vec<u64>.
    ///
    /// Use care when using this directly. If using fixed length structs
    /// always use the `unpack` method, instead.
    fn unpack_unchecked(src: &mut &[u8]) -> Result<Self, ProgramError>;

    /// Returns the size of the byte array required to serialize Self.
    fn size(&self) -> Result<u64, ProgramError>;

    /// Analogue to pack, performing a check on the size of the given byte
    /// array.
    fn unpack(src: &[u8]) -> Result<Self, ProgramError> {
        let mut src_mut = src;
        Pack::unpack_unchecked(&mut src_mut).and_then(|r: Self| {
            if !src_mut.is_empty() {
                return Err(ProgramError::InvalidAccountData);
            }
            Ok(r)
        })
    }

    /// Mutable version of unpack.
    fn unpack_mut<F, U>(input: &mut [u8], f: &mut F) -> Result<U, ProgramError>
    where
        F: FnMut(&mut Self) -> Result<U, ProgramError>,
    {
        let mut t = Self::unpack(input)?;
        let u = f(&mut t)?;
        Self::pack(t, input)?;
        Ok(u)
    }

    /// Unsafe unpack. Doesn't check the size of the given input array.
    fn unpack_unchecked_mut<F, U>(input: &mut [u8], f: &mut F) -> Result<U, ProgramError>
    where
        F: FnMut(&mut Self) -> Result<U, ProgramError>,
    {
        let mut t = Self::unpack_unchecked(&mut input.as_ref())?;
        let u = f(&mut t)?;
        Self::pack(t, input)?;
        Ok(u)
    }
}

/// A convenience macro to easily implement `Pack` for any type that implements
/// serde's Serialize, Deserialize traits.
///
/// When using this, one should consider the performance impact of using
/// Serde and the associated serializer when targeting BPF. The state of this
/// is not entirely clear as of now.
#[macro_export]
macro_rules! packable {
    ($my_struct:ty) => {
        impl Pack for $my_struct {
            fn pack(src: $my_struct, dst: &mut [u8]) -> Result<(), ProgramError> {
                if src.size()? != dst.len() as u64 {
                    return Err(ProgramError::InvalidAccountData);
                }
                serum_common::pack::into_bytes(&src, dst)
            }

            fn unpack_unchecked(src: &mut &[u8]) -> Result<$my_struct, ProgramError> {
                serum_common::pack::from_bytes_mut(src)
            }

            fn size(&self) -> Result<u64, ProgramError> {
                serum_common::pack::bytes_size(&self)
            }
        }
    };
}

pub fn to_bytes<T: ?Sized>(i: &T) -> Result<Vec<u8>, ProgramError>
where
    T: BorshSerialize,
{
    i.try_to_vec().map_err(|_| ProgramError::InvalidAccountData)
}

pub fn into_bytes<T: ?Sized>(i: &T, dst: &mut [u8]) -> Result<(), ProgramError>
where
    T: BorshSerialize,
{
    let mut cursor = std::io::Cursor::new(dst);
    i.serialize(&mut cursor)
        .map_err(|_| ProgramError::InvalidAccountData)
}

pub fn from_bytes<T>(data: &[u8]) -> Result<T, ProgramError>
where
    T: BorshDeserialize,
{
    T::try_from_slice(data).map_err(|_| ProgramError::InvalidAccountData)
}

pub fn from_bytes_mut<T>(data: &mut &[u8]) -> Result<T, ProgramError>
where
    T: BorshDeserialize,
{
    T::deserialize(data).map_err(|_| ProgramError::InvalidAccountData)
}

pub fn bytes_size<T: ?Sized>(value: &T) -> Result<u64, ProgramError>
where
    T: BorshSerialize,
{
    to_bytes(value)
        .map(|s| s.len() as u64)
        .map_err(|_| ProgramError::InvalidAccountData)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate as serum_common;

    #[derive(Clone, Debug, Default, PartialEq, BorshSerialize, BorshDeserialize)]
    struct TestStruct {
        a: u64,
        b: u64,
    }

    packable!(TestStruct);

    #[test]
    fn pack_unpack() {
        let strct = TestStruct::default();
        let mut dst = vec![0; strct.size().unwrap() as usize];
        TestStruct::pack(strct.clone(), &mut dst).unwrap();
        let new_strct = TestStruct::unpack(&dst).unwrap();
        assert_eq!(strct, new_strct)
    }

    #[test]
    fn unpack_too_small() {
        let data = vec![0; 8];
        let r = TestStruct::unpack(&data);
        assert_eq!(r.unwrap_err(), ProgramError::InvalidAccountData);
    }

    #[test]
    fn unpack_too_large() {
        let data = vec![0; 100];
        let r = TestStruct::unpack(&data);
        assert_eq!(r.unwrap_err(), ProgramError::InvalidAccountData);
    }

    #[test]
    fn pack_too_small() {
        let mut data = vec![0; 8];
        let r = TestStruct::pack(Default::default(), &mut data);
        assert_eq!(r.unwrap_err(), ProgramError::InvalidAccountData);
    }

    #[test]
    fn pack_too_large() {
        let mut data = vec![0; 100];
        let r = TestStruct::pack(Default::default(), &mut data);
        assert_eq!(r.unwrap_err(), ProgramError::InvalidAccountData);
    }

    #[derive(Clone, Debug, Default, PartialEq, BorshSerialize, BorshDeserialize)]
    pub struct VarLenStruct {
        a: u64,
        v: Vec<u64>,
    }
    packable!(VarLenStruct);

    #[test]
    fn var_len_struct_unpack_unchecked() {
        let mut data = [0; 100].as_ref();
        let r = VarLenStruct::unpack_unchecked(&mut data);
        assert!(r.is_ok());
        assert_eq!(data.len(), 88);
    }

    #[test]
    fn var_len_struct_unpack_checked() {
        let data = [0; 100].as_ref();
        let r = VarLenStruct::unpack(&data);
        assert_eq!(r.unwrap_err(), ProgramError::InvalidAccountData);
    }
}
