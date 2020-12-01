use serum_pool::context::PoolContext;
use serum_pool_schema::PoolState;
use serum_stake::error::StakeError;
use solana_program::info;
use solana_sdk::account_info::next_account_info;

pub fn handler(ctx: &PoolContext, state: &mut PoolState) -> Result<(), StakeError> {
    info!("handler: initialize_pool");
    assert!(ctx.custom_accounts.len() == 1);
    let acc_infos = &mut ctx.custom_accounts.into_iter();

    // Registry program-derived-address.
    let admin_acc_info = next_account_info(acc_infos)?;

    state.admin_key = Some(admin_acc_info.key.into());

    Ok(())
}
