use num_enum::IntoPrimitive;
use solana_sdk::program_error::ProgramError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum RewardsError {
    #[error(transparent)]
    ProgramError(#[from] ProgramError),
    #[error("{0:?}")]
    ErrorCode(#[from] RewardsErrorCode),
}

#[derive(Debug, IntoPrimitive, Clone, Copy)]
#[repr(u32)]
pub enum RewardsErrorCode {
    WrongSerialization = 0,
    Unknown = 1000,
}

impl std::fmt::Display for RewardsErrorCode {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        <Self as std::fmt::Debug>::fmt(self, fmt)
    }
}

impl std::error::Error for RewardsErrorCode {}

impl std::convert::From<RewardsError> for ProgramError {
    fn from(e: RewardsError) -> ProgramError {
        match e {
            RewardsError::ProgramError(e) => e,
            RewardsError::ErrorCode(c) => ProgramError::Custom(c.into()),
        }
    }
}
