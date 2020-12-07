use num_enum::IntoPrimitive;
use solana_client_gen::solana_sdk::program_error::ProgramError;
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
    NotRentExempt = 1,
    AlreadyInitialized = 2,
    NotInitialized = 3,
    InvalidAccountOwner = 4,
    InvalidRentSysvar = 5,
    InvalidTokenOwner = 6,
    InvalidVaultNonce = 7,
    InvalidVaultAuthority = 8,
    InvalidRegistrar = 9,
    Unauthorized = 10,
    InvalidEntity = 11,
    InvalidLeader = 12,
    EntityNotActive = 13,
    InvalidEventQueueOwner = 14,
    InvalidEventQueue = 15,
    InvalidVault = 16,
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
