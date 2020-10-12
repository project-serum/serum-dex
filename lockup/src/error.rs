use num_enum::IntoPrimitive;
use solana_client_gen::solana_sdk::program_error::ProgramError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum LockupError {
    #[error(transparent)]
    ProgramError(#[from] ProgramError),
    #[error("{0:?}")]
    ErrorCode(#[from] LockupErrorCode),
}

#[derive(Debug, IntoPrimitive, Clone, Copy)]
#[repr(u32)]
pub enum LockupErrorCode {
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
    InsufficientMintBalance = 13,
    Unauthorized = 14,
    MintAlreadyInitialized = 15,
    ReceiptAlreadyInitialized = 16,
    InvalidAccount = 17,
    InvalidVault = 18,
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
    InvalidTimestamp = 30,
    InvalidClock = 31,
    InvalidRentSysvar = 32,
    InvalidMint = 33,
    WrongSafe = 34,
    WrongVestingAccount = 35,
    InvalidDepositAmount = 36,
    InvalidPeriod = 37,
    InsufficientWithdrawalBalance = 38,
    WhitelistAlreadyInitialized = 39,
    AlreadyClaimed = 40,
    InvalidTokenAccountOwner = 41,
    InvalidTokenAccountMint = 42,
    WhitelistFull = 43,
    WhitelistNotFound = 44,
    WhitelistDepositInvariantViolation = 45,
    InvalidRedemptionMint = 46,
    InvalidWhitelist = 47,
    InvalidMintAuthority = 48,
    InvalidMintSupply = 49,
    NotYetClaimed = 50,
    InvalidClockSysvar = 51,
    InsufficientWhitelistBalance = 52,
    WhitelistProgramWrongOwner = 53,
    WhitelistInvalidData = 54,
    InvalidWhitelistEntry = 55,
    WhitelistInvalidProgramId = 56,
    WhitelistEntryAlreadyExists = 57,
    WhitelistSafeMismatch = 58,
    Unknown = 1000,
}

impl std::fmt::Display for LockupErrorCode {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        <Self as std::fmt::Debug>::fmt(self, fmt)
    }
}

impl std::error::Error for LockupErrorCode {}

impl std::convert::From<LockupError> for ProgramError {
    fn from(e: LockupError) -> ProgramError {
        match e {
            LockupError::ProgramError(e) => e,
            LockupError::ErrorCode(c) => ProgramError::Custom(c.into()),
        }
    }
}
