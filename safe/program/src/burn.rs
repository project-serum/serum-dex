use serum_safe::error::SafeError;
//use serum_safe::pack::DynPack;
use solana_sdk::account_info::AccountInfo;
use solana_sdk::info;
use solana_sdk::pubkey::Pubkey;

pub fn handler(_program_id: &Pubkey, _accounts: &[AccountInfo]) -> Result<(), SafeError> {
    info!("handler: burn_locked_srm");
    access_control(AccessControlRequest {})?;
    Ok(())
}

fn access_control(req: AccessControlRequest) -> Result<(), SafeError> {
    info!("access-control: burn");
    let AccessControlRequest {} = req;

    info!("access-control: success");
    Ok(())
}

fn state_transition(req: StateTransitionRequest) -> Result<(), SafeError> {
    info!("state-transition: burn");
    let StateTransitionRequest {} = req;
    info!("state-transition: success");
    Ok(())
}

struct AccessControlRequest {}

struct StateTransitionRequest {}
