use num_enum::IntoPrimitive;
use solana_client_gen::solana_sdk::program_error::ProgramError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SafeError {
    #[error(transparent)]
    ProgramError(#[from] ProgramError),
    #[error("{0:?}")]
    ErrorCode(#[from] SafeErrorCode),
}

#[derive(Debug, IntoPrimitive, Clone, Copy)]
#[repr(u32)]
pub enum SafeErrorCode {
    WrongSerialization = 0,
    NotRentExempt,
    AlreadyInitialized,
    NotOwnedByProgram,
    VestingAccountDataInvalid,
    WrongCoinMint,
    WrongVaultAddress,
    SafeAccountDataInvalid,
    Unknown = 1000,
}

impl std::fmt::Display for SafeErrorCode {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        <Self as std::fmt::Debug>::fmt(self, fmt)
    }
}

impl std::error::Error for SafeErrorCode {}

impl std::convert::From<SafeError> for ProgramError {
    fn from(e: SafeError) -> ProgramError {
        match e {
            SafeError::ProgramError(e) => e,
            SafeError::ErrorCode(c) => ProgramError::Custom(c.into()),
        }
    }
}
