use num_enum::IntoPrimitive;
use solana_client_gen::solana_sdk::program_error::ProgramError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MetaEntityError {
    #[error(transparent)]
    ProgramError(#[from] ProgramError),
    #[error("{0:?}")]
    ErrorCode(#[from] MetaEntityErrorCode),
}

#[derive(Debug, IntoPrimitive, Clone, Copy)]
#[repr(u32)]
pub enum MetaEntityErrorCode {
    WrongSerialization = 0,
    Unauthorized = 1,
    AlreadyInitialized = 2,
    InvalidOwner = 3,
    NotInitialized = 4,
    MetaEntityErrorCode = 5,
    InvalidMessageSize = 6,

    Unknown = 1000,
}

impl std::fmt::Display for MetaEntityErrorCode {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        <Self as std::fmt::Debug>::fmt(self, fmt)
    }
}

impl std::error::Error for MetaEntityErrorCode {}

impl std::convert::From<MetaEntityError> for ProgramError {
    fn from(e: MetaEntityError) -> ProgramError {
        match e {
            MetaEntityError::ProgramError(e) => e,
            MetaEntityError::ErrorCode(c) => ProgramError::Custom(c.into()),
        }
    }
}
