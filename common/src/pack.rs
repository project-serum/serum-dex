//! pack.rs defines utilities for serializing Solana accounts to/from bytes.

use solana_sdk::program_error::ProgramError;

/// The Pack trait defines Account serialization for Solana programs.
///
/// If possible, don't use `*_unchecked` methods.
pub trait Pack<'a>: serde::Serialize + serde::Deserialize<'a> {
    /// Serializes `src` into `dst`. The size of the serialization and
    /// dst must be equal.
    fn pack(src: Self, dst: &mut [u8]) -> Result<(), ProgramError>;

    /// Deserializes `src` into Self. The deserialized object need not
    /// use all the bytes in `src`. This is the case, for example,
    /// when encoding variable length data, say, when one has a src
    /// array of all zeroes, and a Self that has a Vec<u64>.
    ///
    /// Use care when using this directly. If using fixed length structs
    /// always use the `unpack` method, instead.
    fn unpack_unchecked(src: &[u8]) -> Result<Self, ProgramError>;

    /// Returns the size of the byte array required to serialize Self.
    fn size(&self) -> Result<u64, ProgramError>;

    /// Analogue to pack, performing a check on the size of the given byte
    /// array.
    fn unpack(src: &[u8]) -> Result<Self, ProgramError> {
        Pack::unpack_unchecked(src).and_then(|r: Self| {
            if r.size()? != src.len() as u64 {
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
        let mut t = Self::unpack_unchecked(input)?;
        let u = f(&mut t)?;
        Self::pack(t, input)?;
        Ok(u)
    }
}

/// A convenience macro to easily implement `Pack` for any type that implements
/// serde's Serialize, Deserialize traits.
#[macro_export]
macro_rules! packable {
    ($my_struct:ty) => {
        use serum_common::pack::Pack;
        use solana_client_gen::solana_sdk::program_error::ProgramError;

        impl<'a> Pack<'a> for $my_struct {
            fn pack(src: $my_struct, dst: &mut [u8]) -> Result<(), ProgramError> {
                if src.size()? != dst.len() as u64 {
                    return Err(ProgramError::InvalidAccountData);
                }
                serum_common::pack::into_bytes(&src, dst)
            }
            fn unpack_unchecked(src: &[u8]) -> Result<$my_struct, ProgramError> {
                serum_common::pack::from_bytes::<$my_struct>(src)
            }
            fn size(&self) -> Result<u64, ProgramError> {
                serum_common::pack::bytes_size(&self)
            }
        }
    };
}

pub fn to_bytes<T: ?Sized>(i: &T) -> Result<Vec<u8>, ProgramError>
where
    T: serde::Serialize,
{
    bincode::serialize(i).map_err(|_| ProgramError::InvalidAccountData)
}

pub fn into_bytes<T: ?Sized>(i: &T, dst: &mut [u8]) -> Result<(), ProgramError>
where
    T: serde::Serialize,
{
    let cursor = std::io::Cursor::new(dst);
    bincode::serialize_into(cursor, i).map_err(|_| ProgramError::InvalidAccountData)
}
pub fn from_bytes<'a, T>(data: &'a [u8]) -> Result<T, ProgramError>
where
    T: serde::de::Deserialize<'a>,
{
    bincode::deserialize(data).map_err(|_| ProgramError::InvalidAccountData)
}

pub fn bytes_size<T: ?Sized>(value: &T) -> Result<u64, ProgramError>
where
    T: serde::Serialize,
{
    bincode::serialized_size(value).map_err(|_| ProgramError::InvalidAccountData)
}

#[cfg(test)]
mod tests {
    use crate as serum_common;

    #[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
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
        let result = TestStruct::unpack(&data);
        match result {
            Ok(_) => panic!("expect error"),
            Err(e) => assert_eq!(e, ProgramError::InvalidAccountData),
        }
    }

    #[test]
    fn unpack_too_large() {
        let data = vec![0; 100];
        let result = TestStruct::unpack(&data);
        match result {
            Ok(_) => panic!("expect error"),
            Err(e) => assert_eq!(e, ProgramError::InvalidAccountData),
        }
    }

    #[test]
    fn pack_too_small() {
        let mut data = vec![0; 8];
        let result = TestStruct::pack(Default::default(), &mut data);
        match result {
            Ok(_) => panic!("expect error"),
            Err(e) => assert_eq!(e, ProgramError::InvalidAccountData),
        }
    }

    #[test]
    fn pack_too_large() {
        let mut data = vec![0; 100];
        let result = TestStruct::pack(Default::default(), &mut data);
        match result {
            Ok(_) => panic!("expect error"),
            Err(e) => assert_eq!(e, ProgramError::InvalidAccountData),
        }
    }
}
