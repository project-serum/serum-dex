use solana_client_gen::solana_sdk::program_error::ProgramError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum RegistryError {
    #[error(transparent)]
    ProgramError(#[from] ProgramError),
    #[error("{0:?}")]
    ErrorCode(#[from] RegistryErrorCode),
}

#[derive(Debug, Clone, Copy)]
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
    EntityLeaderMismatch = 26,
    EntityMemberMismatch = 27,
    CheckedFailure = 28,
    AlreadyBurned = 29,
    WithdrawalTimelockNotPassed = 30,
    InvalidAssetsLen = 31,
    DelegateAccountsNotProvided = 32,
    PoolMismatch = 33,
    MegaPoolMismatch = 34,
    PoolProgramIdMismatch = 35,
    SharedMemoryMismatch = 36,
    InsufficientBalance = 37,
    InvalidVault = 38,
    InvalidPoolAccounts = 39,
    RetbufError = 40,
    InvalidStakeTokenOwner = 41,
    InvalidStakeTokenDelegate = 42,
    GenerationEntityMismatch = 43,
    InvalidGenerationNumber = 44,
    EntityMaxStake = 45,
    StakeNotEmpty = 46,
    SptDelegateAlreadySet = 47,
    RingInvalidMessageSize = 48,
    RewardQAlreadyOwned = 49,
    InvalidRewardQueueAuthority = 50,
    InvalidPoolTokenMint = 51,
    InvalidCursor = 52,
    AlreadyProcessedCursor = 53,
    MemberRegistrarMismatch = 54,
    VendorRegistrarMismatch = 55,
    RegistrarRewardQMismatch = 56,
    IneligibleReward = 57,
    InvalidExpiry = 58,
    InvalidEndTs = 59,
    DepositorOwnerDelegateMismatch = 60,
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
            RegistryError::ErrorCode(c) => ProgramError::Custom(c as u32),
        }
    }
}
