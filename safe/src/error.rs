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
    NotRentExempt = 1,
    AlreadyInitialized = 2,
    NotInitialized = 3,
    NotOwnedByProgram = 4,
    VestingAccountDataInvalid = 5,
    WrongCoinMint = 6,
    SafeDataInvalid = 8,
    NotSignedByAuthority = 11,
    WrongNumberOfAccounts = 12,
    InsufficientBalance = 13,
    Unauthorized = 14,
    LsrmMintAlreadyInitialized = 15,
    LsrmReceiptAlreadyInitialized = 16,
    InvalidAccount = 17,
    WrongVault = 18,
    InvalidVaultNonce = 19,
    InvalidReceipt = 20,
    AlreadyBurned = 21,
    InvalidAccountOwner = 22,
    UnauthorizedReceipt = 23,
    TokenAccountAlreadyInitialized = 24,
    TokenAccountOwnerMismatch = 25,
    InvalidTokenProgram = 26,
    InvalidSerialization = 27,
    SizeNotAvailable = 28,
    UnitializedTokenMint = 29,
    InvalidVestingSlots = 30,
    InvalidVestingAmounts = 31,
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
