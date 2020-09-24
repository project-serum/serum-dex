use serde::{Deserialize, Serialize};

/// InstructionCoder is the trait that must be implemented to user
/// custom serialization with the main macro. If a coder is not
/// provided, the `DefaultCoder` will be used.
pub trait InstructionCoder<'a, T: ?Sized + Serialize + Deserialize<'a>> {
    fn to_bytes(i: T) -> Vec<u8>;
    fn from_bytes(data: &'a [u8]) -> Result<T, ()>;
}

pub struct DefaultCoder;
impl<'a, T: ?Sized + serde::Serialize + serde::Deserialize<'a>> InstructionCoder<'a, T>
    for DefaultCoder
{
    fn to_bytes(i: T) -> Vec<u8> {
        serum_common::pack::to_bytes(&i).expect("instruction must be serializable")
    }
    fn from_bytes(data: &'a [u8]) -> Result<T, ()> {
        serum_common::pack::from_bytes(data).map_err(|_| ())
    }
}
