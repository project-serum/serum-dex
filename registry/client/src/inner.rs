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
    max_stake_per_entity: u64,
) -> Result<(Signature, Pubkey, Pubkey, u8, Pubkey, Pubkey), InnerClientError> {
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

    let pool_vault = rpc::create_token_account(
        client.rpc(),
        mint,
        &registrar_vault_authority,
        client.payer(),
    )
    .map_err(|e| InnerClientError::RawError(e.to_string()))?
    .pubkey();
    let mega_pool_vault = rpc::create_token_account(
        client.rpc(),
        mega_mint,
        &registrar_vault_authority,
        client.payer(),
    )
    .map_err(|e| InnerClientError::RawError(e.to_string()))?
    .pubkey();
    let decimals = 6; // TODO: decide on this.
    let pool_mint = rpc::new_mint(
        client.rpc(),
        client.payer(),
        &registrar_vault_authority,
        decimals,
    )
    .map_err(|e| InnerClientError::RawError(e.to_string()))?
    .0
    .pubkey();

    let mega_pool_mint = rpc::new_mint(
        client.rpc(),
        client.payer(),
        &registrar_vault_authority,
        decimals,
    )
    .map_err(|e| InnerClientError::RawError(e.to_string()))?
    .0
    .pubkey();

    // Build the instructions.
    let ixs = {
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
                // Deposit vaults.
                AccountMeta::new_readonly(srm_vault.pubkey(), false),
                AccountMeta::new_readonly(msrm_vault.pubkey(), false),
                // Pool vaults.
                AccountMeta::new_readonly(pool_vault, false),
                AccountMeta::new_readonly(mega_pool_vault, false),
                // Pool mints.
                AccountMeta::new_readonly(pool_mint, false),
                AccountMeta::new_readonly(mega_pool_mint, false),
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

        vec![
            create_reward_event_q_instr,
            create_registrar_acc_instr,
            initialize_registrar_instr,
        ]
    };

    let (recent_hash, _fee_calc) = client
        .rpc()
        .get_recent_blockhash()
        .map_err(|e| InnerClientError::RawError(e.to_string()))?;
    let tx = Transaction::new_signed_with_payer(
        &ixs,
        Some(&client.payer().pubkey()),
        &vec![client.payer(), &reward_event_q, &registrar_kp],
        recent_hash,
    );
    let sig = client
        .rpc()
        .send_and_confirm_transaction_with_spinner_and_config(
            &tx,
            client.options().commitment,
            client.options().tx,
        )
        .map_err(InnerClientError::RpcError)?;
    Ok((
        sig,
        registrar_kp.pubkey(),
        reward_event_q.pubkey(),
        nonce,
        pool_vault,
        mega_pool_vault,
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
