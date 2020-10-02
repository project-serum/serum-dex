use serum_registry::error::{RegistryError, RegistryErrorCode};
use solana_sdk::account_info::AccountInfo;
use solana_sdk::info;
use solana_sdk::pubkey::Pubkey;

pub fn handler<'a>(
    program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'a>],
) -> Result<(), RegistryError> {
    // todo
    info!("handler: complete_stake_withdrawl");

    access_control(AccessControlRequest {})?;

    state_transition(StateTransitionRequest {})
}

fn access_control(req: AccessControlRequest) -> Result<(), RegistryError> {
    // todo

    info!("access-control: complete_stake_withdrawal");

    let AccessControlRequest {} = req;

    info!("access-control: success");

    Ok(())
}

fn state_transition(req: StateTransitionRequest) -> Result<(), RegistryError> {
    // todo
    info!("state-transition: complete_stake_withdrawal");

    let StateTransitionRequest {} = req;

    info!("state-transition: success");

    Ok(())
}

struct AccessControlRequest {}

struct StateTransitionRequest {}
