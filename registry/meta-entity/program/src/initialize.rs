use serum_common::pack::Pack;
use serum_meta_entity::accounts::Metadata;
use serum_meta_entity::error::{MetaEntityError, MetaEntityErrorCode};
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::info;
use solana_sdk::pubkey::Pubkey;

pub fn handler(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    entity: Pubkey,
    authority: Pubkey,
    name: String,
    about: String,
    image_url: String,
    chat: Pubkey,
) -> Result<(), MetaEntityError> {
    info!("handler: initialize");

    let acc_infos = &mut accounts.iter();

    let metadata_acc_info = next_account_info(acc_infos)?;
    if !metadata_acc_info.is_signer {
        return Err(MetaEntityErrorCode::Unauthorized.into());
    }
    if metadata_acc_info.owner != program_id {
        return Err(MetaEntityErrorCode::InvalidOwner.into());
    }

    Metadata::unpack_unchecked_mut(
        &mut metadata_acc_info.try_borrow_mut_data()?,
        &mut |metadata: &mut Metadata| {
            if metadata.initialized {
                return Err(MetaEntityError::ErrorCode(
                    MetaEntityErrorCode::AlreadyInitialized,
                ))?;
            }
            metadata.entity = entity;
            metadata.authority = authority;
            metadata.initialized = true;
            metadata.name = name.clone();
            metadata.about = about.clone();
            metadata.image_url = image_url.clone();
            metadata.chat = chat;
            Ok(())
        },
    )?;

    Ok(())
}
