use num_enum::IntoPrimitive;
use solana_client_gen::solana_sdk::program_error::ProgramError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum RegistryError {
    #[error(transparent)]
    ProgramError(#[from] ProgramError),
    #[error("{0:?}")]
    ErrorCode(#[from] RegistryErrorCode),
}

#[derive(Debug, IntoPrimitive, Clone, Copy)]
#[repr(u32)]
pub enum RegistryErrorCode {
    WrongSerialization = 1,
    NotReadySeeNextMajorVersion = 2,
    MustBeDelegated = 3,
    Unknown = 1000,
}

impl std::fmt::Display for RegistryErrorCode {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        <Self as std::fmt::Debug>::fmt(self, fmt)
    }
}

impl std::error::Error for RegistryErrorCode {}

impl std::convert::From<RegistryError> for ProgramError {
    fn from(e: RegistryError) -> ProgramError {
        match e {
            RegistryError::ProgramError(e) => e,
            RegistryError::ErrorCode(c) => ProgramError::Custom(c.into()),
        }
    }
}
