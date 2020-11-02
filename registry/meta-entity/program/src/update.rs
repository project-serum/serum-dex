use serum_common::pack::Pack;
use serum_meta_entity::accounts::Metadata;
use serum_meta_entity::error::{MetaEntityError, MetaEntityErrorCode};
use solana_sdk::account_info::{next_account_info, AccountInfo};
use solana_sdk::info;
use solana_sdk::pubkey::Pubkey;

pub fn handler(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    name: Option<String>,
    about: Option<String>,
    image_url: Option<String>,
    chat: Option<Pubkey>,
) -> Result<(), MetaEntityError> {
    info!("handler: update");

    let acc_infos = &mut accounts.iter();

    let metadata_acc_info = next_account_info(acc_infos)?;
    let authority_acc_info = next_account_info(acc_infos)?;

    if metadata_acc_info.owner != program_id {
        return Err(MetaEntityErrorCode::InvalidOwner.into());
    }
    if !authority_acc_info.is_signer {
        return Err(MetaEntityErrorCode::Unauthorized.into());
    }

    Metadata::unpack_unchecked_mut(
        &mut metadata_acc_info.try_borrow_mut_data()?,
        &mut |metadata: &mut Metadata| {
            if !metadata.initialized {
                return Err(MetaEntityError::ErrorCode(
                    MetaEntityErrorCode::NotInitialized,
                ))?;
            }

            if let Some(name) = name.clone() {
                metadata.name = name;
            }
            if let Some(about) = about.clone() {
                metadata.about = about;
            }
            if let Some(image_url) = image_url.clone() {
                metadata.image_url = image_url;
            }
            if let Some(chat) = chat {
                metadata.chat = chat;
            }
            Ok(())
        },
    )?;

    Ok(())
}
