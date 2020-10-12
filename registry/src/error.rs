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
    InvalidClockSysvar = 4,
    InvalidAccountOwner = 5,
    NotInitialized = 6,
    Unauthorized = 7,
    StaleStakeNeedsWithdrawal = 8,
    InvalidOwner = 9,
    EntityRegistrarMismatch = 10,
    MemberEntityMismatch = 11,
    EntityNotActivated = 12,
    RegistrarVaultMismatch = 13,
    MemberDelegateMismatch = 14,
    MemberBeneficiaryMismatch = 15,
    InvalidRentSysvar = 16,
    NotRentExempt = 17,
    AlreadyInitialized = 18,
    InvalidVaultAuthority = 19,
    InvalidVaultNonce = 20,
    DelegateInUse = 21,
    InvalidCapabilityId = 22,
    InsufficientStakeIntentBalance = 23,
    InvalidMemberDelegateOwner = 24,
    InvalidTokenAuthority = 25,
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
