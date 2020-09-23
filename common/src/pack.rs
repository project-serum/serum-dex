use solana_sdk::program_error::ProgramError;

pub trait Pack<'a>: serde::Serialize + serde::Deserialize<'a> {
    fn pack(src: Self, dst: &mut [u8]) -> Result<(), ProgramError>;
    fn unpack(src: &[u8]) -> Result<Self, ProgramError>;
    fn size(&self) -> Result<u64, ProgramError>;

    fn unpack_mut<F, U>(input: &mut [u8], f: &mut F) -> Result<U, ProgramError>
    where
        F: FnMut(&mut Self) -> Result<U, ProgramError>,
    {
        let mut t = Self::unpack(input)?;
        let u = f(&mut t)?;
        Self::pack(t, input)?;
        Ok(u)
    }
}

#[macro_export]
macro_rules! packable {
    ($my_struct:ty) => {
        use serum_common::pack::Pack;
        use solana_client_gen::solana_sdk::program_error::ProgramError;

        impl<'a> Pack<'a> for $my_struct {
            fn pack(src: $my_struct, dst: &mut [u8]) -> Result<(), ProgramError> {
                serum_common::pack::into_bytes(&src, dst)
            }
            fn unpack(src: &[u8]) -> Result<$my_struct, ProgramError> {
                serum_common::pack::from_bytes::<$my_struct>(src)
                    .map_err(|_| ProgramError::InvalidAccountData)
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
