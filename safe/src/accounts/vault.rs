use solana_client_gen::solana_sdk::pubkey::Pubkey;

pub struct SrmVault;
impl SrmVault {
    /// address returns the program-derived-address for the SrmVault account
    /// holding SRM SPL tokens on behalf of the contract.
    ///
    /// For more information on program,
    /// see https://docs.solana.com/implemented-proposals/program-derived-addresses.
    pub fn program_derived_address(program_id: &Pubkey, safe_account_key: &Pubkey) -> Pubkey {
        Pubkey::create_program_address(&[safe_account_key.as_ref()], program_id)
            .expect("SrmVault must always have an address")
    }
}
