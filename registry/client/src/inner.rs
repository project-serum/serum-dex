use crate::{MEGA_POOL_STATE_SIZE, POOL_STATE_SIZE};
use serum_common::client::rpc;
use serum_common::pack::Pack;
use serum_registry::accounts;
use serum_registry::accounts::reward_queue::{RewardEventQueue, Ring};
use serum_registry::client::{Client as InnerClient, ClientError as InnerClientError};
use solana_client_gen::prelude::*;
use solana_client_gen::solana_sdk;
use solana_client_gen::solana_sdk::instruction::{AccountMeta, Instruction};
use solana_client_gen::solana_sdk::pubkey::Pubkey;
use solana_client_gen::solana_sdk::system_instruction;

pub fn initialize(
    client: &InnerClient,
    mint: &Pubkey,
    mega_mint: &Pubkey,
    registrar_authority: &Pubkey,
    withdrawal_timelock: i64,
    deactivation_timelock_premium: i64,
    reward_activation_threshold: u64,
    pool_program_id: &Pubkey,
    pool_token_decimals: u8,
    max_stake_per_entity: u64,
) -> Result<
    (
        Signature,
        Signature,
        Pubkey,
        Pubkey,
        u8,
        Pubkey,
        u8,
        Pubkey,
        u8,
    ),
    InnerClientError,
> {
    let reward_event_q = Keypair::generate(&mut OsRng);
    let registrar_kp = Keypair::generate(&mut OsRng);
    let (registrar_vault_authority, nonce) =
        Pubkey::find_program_address(&[registrar_kp.pubkey().as_ref()], client.program());

    // Create and initialize the vaults, both owned by the program-derived-address.
    let srm_vault = rpc::create_token_account(
        client.rpc(),
        mint,
        &registrar_vault_authority,
        client.payer(),
    )
    .map_err(|e| InnerClientError::RawError(e.to_string()))?;
    let msrm_vault = rpc::create_token_account(
        client.rpc(),
        mega_mint,
        &registrar_vault_authority,
        client.payer(),
    )
    .map_err(|e| InnerClientError::RawError(e.to_string()))?;

    let pool_state_kp = Keypair::generate(&mut OsRng);
    let (pool_vault_authority, pool_vault_nonce) =
        Pubkey::find_program_address(&[pool_state_kp.pubkey().as_ref()], pool_program_id);

    let mega_pool_state_kp = Keypair::generate(&mut OsRng);
    let (mega_pool_vault_authority, mega_pool_vault_nonce) =
        Pubkey::find_program_address(&[mega_pool_state_kp.pubkey().as_ref()], pool_program_id);

    // Build the instructions.
    let (create_instrs, init_instrs) = {
        let create_registrar_acc_instr = {
            let lamports = client
                .rpc()
                .get_minimum_balance_for_rent_exemption(*accounts::registrar::SIZE as usize)
                .map_err(InnerClientError::RpcError)?;
            system_instruction::create_account(
                &client.payer().pubkey(),
                &registrar_kp.pubkey(),
                lamports,
                *accounts::registrar::SIZE,
                client.program(),
            )
        };
        // Mint pool.

        let create_pool_acc_instr = {
            let lamports = client
                .rpc()
                .get_minimum_balance_for_rent_exemption(*POOL_STATE_SIZE as usize)
                .map_err(InnerClientError::RpcError)?;
            system_instruction::create_account(
                &client.payer().pubkey(),
                &pool_state_kp.pubkey(),
                lamports,
                *POOL_STATE_SIZE,
                pool_program_id,
            )
        };
        let create_mega_pool_acc_instr = {
            let lamports = client
                .rpc()
                .get_minimum_balance_for_rent_exemption(*MEGA_POOL_STATE_SIZE as usize)
                .map_err(InnerClientError::RpcError)?;
            system_instruction::create_account(
                &client.payer().pubkey(),
                &mega_pool_state_kp.pubkey(),
                lamports,
                *MEGA_POOL_STATE_SIZE,
                pool_program_id,
            )
        };
        let initialize_pool_instr = {
            let pool_asset_mint = mint;
            let pool_asset_vault = rpc::create_token_account(
                client.rpc(),
                pool_asset_mint,
                &pool_vault_authority,
                client.payer(),
            )
            .map_err(|e| InnerClientError::RawError(e.to_string()))?;
            let (pool_token_mint, _tx_sig) = rpc::new_mint(
                client.rpc(),
                client.payer(),
                &pool_vault_authority,
                pool_token_decimals,
            )
            .map_err(|e| InnerClientError::RawError(e.to_string()))?;
            serum_stake::instruction::initialize(
                pool_program_id,
                &pool_state_kp.pubkey(),
                &pool_token_mint.pubkey(),
                vec![&pool_asset_vault.pubkey()],
                &pool_vault_authority,
                &registrar_vault_authority,
                pool_vault_nonce,
            )
        };
        // Mega pool has both SRM and MSRM in the basket.
        let initialize_mega_pool_instr = {
            let pool_asset_mint = mint;
            let mega_pool_asset_mint = mega_mint;
            let pool_asset_vault = rpc::create_token_account(
                client.rpc(),
                pool_asset_mint,
                &mega_pool_vault_authority,
                client.payer(),
            )
            .map_err(|e| InnerClientError::RawError(e.to_string()))?;
            let mega_pool_asset_vault = rpc::create_token_account(
                client.rpc(),
                mega_pool_asset_mint,
                &mega_pool_vault_authority,
                client.payer(),
            )
            .map_err(|e| InnerClientError::RawError(e.to_string()))?;
            let (mega_pool_token_mint, _tx_sig) = rpc::new_mint(
                client.rpc(),
                client.payer(),
                &mega_pool_vault_authority,
                pool_token_decimals,
            )
            .map_err(|e| InnerClientError::RawError(e.to_string()))?;
            serum_stake::instruction::initialize(
                pool_program_id,
                &mega_pool_state_kp.pubkey(),
                &mega_pool_token_mint.pubkey(),
                vec![&pool_asset_vault.pubkey(), &mega_pool_asset_vault.pubkey()],
                &mega_pool_vault_authority,
                &registrar_vault_authority,
                mega_pool_vault_nonce,
            )
        };

        let create_reward_event_q_instr = {
            let data_size = RewardEventQueue::buffer_size(RewardEventQueue::RING_CAPACITY);
            let lamports = client
                .rpc()
                .get_minimum_balance_for_rent_exemption(data_size)?;
            solana_sdk::system_instruction::create_account(
                &client.payer().pubkey(),
                &reward_event_q.pubkey(),
                lamports,
                data_size as u64,
                client.program(),
            )
        };

        let initialize_registrar_instr = {
            let accounts = [
                AccountMeta::new(registrar_kp.pubkey(), false),
                AccountMeta::new_readonly(srm_vault.pubkey(), false),
                AccountMeta::new_readonly(msrm_vault.pubkey(), false),
                AccountMeta::new_readonly(pool_state_kp.pubkey(), false),
                AccountMeta::new_readonly(mega_pool_state_kp.pubkey(), false),
                AccountMeta::new_readonly(*pool_program_id, false),
                AccountMeta::new(reward_event_q.pubkey(), false),
                AccountMeta::new_readonly(solana_sdk::sysvar::rent::id(), false),
            ];
            serum_registry::instruction::initialize(
                *client.program(),
                &accounts,
                *registrar_authority,
                nonce,
                withdrawal_timelock,
                deactivation_timelock_premium,
                reward_activation_threshold,
                max_stake_per_entity,
            )
        };

        (
            vec![
                create_reward_event_q_instr,
                create_registrar_acc_instr,
                create_pool_acc_instr,
                create_mega_pool_acc_instr,
            ],
            vec![
                initialize_pool_instr,
                initialize_mega_pool_instr,
                initialize_registrar_instr,
            ],
        )
    };

    // The transaction is too big, so we break it up into two.
    //
    // Technically this is not safe, since someone can claim the created
    // accounts before they're initialized. However, the programs check for
    // this and will reject initialization if so. Furthermore, we do a one
    // time initialization so in practice it's not a problem. The worse,
    // that can hapen is that someon steals our accounts, in which case we4
    // lose the SOL required to create them, and then we retry the
    // initialzation with new accounts.

    let (recent_hash, _fee_calc) = client
        .rpc()
        .get_recent_blockhash()
        .map_err(|e| InnerClientError::RawError(e.to_string()))?;
    let tx_1 = Transaction::new_signed_with_payer(
        &create_instrs,
        Some(&client.payer().pubkey()),
        &vec![
            client.payer(),
            &reward_event_q,
            &registrar_kp,
            &pool_state_kp,
            &mega_pool_state_kp,
        ],
        recent_hash,
    );
    let tx_2 = Transaction::new_signed_with_payer(
        &init_instrs,
        Some(&client.payer().pubkey()),
        &[client.payer()],
        recent_hash,
    );
    let sig_1 = client
        .rpc()
        .send_and_confirm_transaction_with_spinner_and_config(
            &tx_1,
            client.options().commitment,
            client.options().tx,
        )
        .map_err(InnerClientError::RpcError)?;
    let sig_2 = client
        .rpc()
        .send_and_confirm_transaction_with_spinner_and_config(
            &tx_2,
            client.options().commitment,
            client.options().tx,
        )
        .map_err(InnerClientError::RpcError)?;
    Ok((
        sig_1,
        sig_2,
        registrar_kp.pubkey(),
        reward_event_q.pubkey(),
        nonce,
        pool_state_kp.pubkey(),
        pool_vault_nonce,
        mega_pool_state_kp.pubkey(),
        mega_pool_vault_nonce,
    ))
}

pub fn create_entity(
    client: &InnerClient,
    registrar: Pubkey,
    leader_kp: &Keypair,
    name: String,
    about: String,
    image_url: String,
    meta_entity_program_id: Pubkey,
) -> Result<(Signature, Pubkey), InnerClientError> {
    let entity_kp = Keypair::generate(&mut OsRng);
    let create_acc_instr = {
        let lamports = client
            .rpc()
            .get_minimum_balance_for_rent_exemption(*accounts::entity::SIZE as usize)
            .map_err(InnerClientError::RpcError)?;
        system_instruction::create_account(
            &client.payer().pubkey(),
            &entity_kp.pubkey(),
            lamports,
            *accounts::entity::SIZE,
            client.program(),
        )
    };

    let accounts = [
        AccountMeta::new(entity_kp.pubkey(), false),
        AccountMeta::new_readonly(leader_kp.pubkey(), true),
        AccountMeta::new_readonly(registrar, false),
        AccountMeta::new_readonly(solana_sdk::sysvar::rent::ID, false),
    ];

    let metadata = Keypair::generate(&mut OsRng);
    let mqueue = Keypair::generate(&mut OsRng);
    let create_entity_instr =
        serum_registry::instruction::create_entity(*client.program(), &accounts, metadata.pubkey());

    let create_md_instrs = create_metadata_instructions(
        client.rpc(),
        &client.payer().pubkey(),
        &metadata,
        &mqueue,
        &meta_entity_program_id,
        &entity_kp.pubkey(),
        name,
        about,
        image_url,
    );
    let mut instructions = create_md_instrs;
    instructions.extend_from_slice(&[create_acc_instr, create_entity_instr]);

    let signers = vec![leader_kp, &metadata, &mqueue, &entity_kp, client.payer()];
    let (recent_hash, _fee_calc) = client.rpc().get_recent_blockhash()?;

    let tx = Transaction::new_signed_with_payer(
        &instructions,
        Some(&client.payer().pubkey()),
        &signers,
        recent_hash,
    );

    client
        .rpc()
        .send_and_confirm_transaction_with_spinner_and_config(
            &tx,
            client.options().commitment,
            client.options().tx,
        )
        .map_err(InnerClientError::RpcError)
        .map(|sig| (sig, entity_kp.pubkey()))
}

// todo: remove
pub fn member_seed() -> &'static str {
    "srm:registry:member"
}

fn create_metadata_instructions(
    client: &RpcClient,
    payer: &Pubkey,
    metadata: &Keypair,
    mqueue: &Keypair,
    program_id: &Pubkey,
    entity: &Pubkey,
    name: String,
    about: String,
    image_url: String,
) -> Vec<Instruction> {
    let md = serum_meta_entity::accounts::Metadata {
        initialized: false,
        entity: Pubkey::new_from_array([0; 32]),
        authority: *payer,
        name: name.clone(),
        about: about.clone(),
        image_url: image_url.clone(),
        chat: Pubkey::new_from_array([0; 32]),
    };
    let metadata_size = md.size().unwrap();
    let lamports = client
        .get_minimum_balance_for_rent_exemption(metadata_size as usize)
        .unwrap();
    let create_metadata_instr = solana_sdk::system_instruction::create_account(
        payer,
        &metadata.pubkey(),
        lamports,
        metadata_size as u64,
        program_id,
    );

    let mqueue_size = serum_meta_entity::accounts::MQueue::SIZE;
    let lamports = client
        .get_minimum_balance_for_rent_exemption(mqueue_size)
        .unwrap();
    let create_mqueue_instr = solana_sdk::system_instruction::create_account(
        payer,
        &mqueue.pubkey(),
        lamports,
        mqueue_size as u64,
        program_id,
    );

    let initialize_metadata_instr = {
        let accounts = vec![AccountMeta::new(metadata.pubkey(), false)];
        let instr = serum_meta_entity::instruction::MetaEntityInstruction::Initialize {
            entity: *entity,
            authority: *payer,
            name,
            about,
            image_url,
            chat: mqueue.pubkey(),
        };
        let mut data = vec![0u8; instr.size().unwrap() as usize];
        serum_meta_entity::instruction::MetaEntityInstruction::pack(instr, &mut data).unwrap();
        Instruction {
            program_id: *program_id,
            accounts,
            data,
        }
    };

    vec![
        create_metadata_instr,
        create_mqueue_instr,
        initialize_metadata_instr,
    ]
}
