use num_enum::IntoPrimitive;
use solana_client_gen::solana_sdk::program_error::ProgramError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum StakeError {
    #[error(transparent)]
    ProgramError(#[from] ProgramError),
    #[error("{0:?}")]
    ErrorCode(#[from] StakeErrorCode),
}

#[derive(Debug, IntoPrimitive, Clone, Copy)]
#[repr(u32)]
pub enum StakeErrorCode {
    WrongSerialization = 0,
    Unauthorized = 1,
    InvalidU64 = 2,
    InvalidState = 3,
    FailedCast = 4,
    Unknown = 1000,
}

impl std::fmt::Display for StakeErrorCode {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        <Self as std::fmt::Debug>::fmt(self, fmt)
    }
}

impl std::error::Error for StakeErrorCode {}

impl std::convert::From<StakeError> for ProgramError {
    fn from(e: StakeError) -> ProgramError {
        match e {
            StakeError::ProgramError(e) => e,
            StakeError::ErrorCode(c) => ProgramError::Custom(c.into()),
        }
    }
}
