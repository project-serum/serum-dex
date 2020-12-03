use serum_meta_entity::accounts::mqueue::Ring;
use serum_meta_entity::accounts::{MQueue, Message};
use serum_meta_entity::error::MetaEntityError;
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::info;
use solana_sdk::pubkey::Pubkey;

pub fn handler(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    msg: Message,
) -> Result<(), MetaEntityError> {
    info!("handler: send_message");
    let acc_infos = &mut accounts.iter();

    let mqueue_acc_info = next_account_info(acc_infos)?;
    let mqueue = MQueue::from(mqueue_acc_info.data.clone());

    mqueue.append(&msg)?;

    Ok(())
}
