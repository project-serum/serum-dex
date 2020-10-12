use serum_registry::accounts;
use serum_registry::accounts::Watchtower;
use serum_registry::client::{Client as InnerClient, ClientError as InnerClientError};
use solana_client_gen::prelude::*;
use solana_client_gen::solana_sdk;
use solana_client_gen::solana_sdk::instruction::AccountMeta;
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
) -> Result<(Signature, Pubkey, u8), InnerClientError> {
    let registrar_kp = Keypair::generate(&mut OsRng);
    let (registrar_vault_authority, nonce) =
        Pubkey::find_program_address(&[registrar_kp.pubkey().as_ref()], client.program());

    // Create and initialize the vaults, both owned by the program-derived-address.
    let srm_vault = serum_common::client::rpc::create_token_account(
        client.rpc(),
        mint,
        &registrar_vault_authority,
        client.payer(),
    )
    .map_err(|e| InnerClientError::RawError(e.to_string()))?;
    let msrm_vault = serum_common::client::rpc::create_token_account(
        client.rpc(),
        mega_mint,
        &registrar_vault_authority,
        client.payer(),
    )
    .map_err(|e| InnerClientError::RawError(e.to_string()))?;

    // Now build the final transaction.
    let instructions = {
        let create_safe_acc_instr = {
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
        let accounts = [
            AccountMeta::new(registrar_kp.pubkey(), false),
            AccountMeta::new_readonly(srm_vault.pubkey(), false),
            AccountMeta::new_readonly(msrm_vault.pubkey(), false),
            AccountMeta::new_readonly(solana_sdk::sysvar::rent::id(), false),
        ];

        let initialize_instr = serum_registry::instruction::initialize(
            *client.program(),
            &accounts,
            *registrar_authority,
            nonce,
            withdrawal_timelock,
            deactivation_timelock_premium,
            reward_activation_threshold,
        );
        vec![create_safe_acc_instr, initialize_instr]
    };

    let tx = {
        let (recent_hash, _fee_calc) = client
            .rpc()
            .get_recent_blockhash()
            .map_err(|e| InnerClientError::RawError(e.to_string()))?;
        let signers = vec![client.payer(), &registrar_kp];
        Transaction::new_signed_with_payer(
            &instructions,
            Some(&client.payer().pubkey()),
            &signers,
            recent_hash,
        )
    };

    // Execute the transaction.
    client
        .rpc()
        .send_and_confirm_transaction_with_spinner_and_config(
            &tx,
            client.options().commitment,
            client.options().tx,
        )
        .map_err(InnerClientError::RpcError)
        .map(|sig| (sig, registrar_kp.pubkey(), nonce))
}

pub fn create_entity_derived(
    client: &InnerClient,
    registrar: Pubkey,
    leader_kp: &Keypair,
    capabilities: u32,
    stake_kind: serum_registry::accounts::StakeKind,
) -> Result<(Signature, Pubkey), InnerClientError> {
    let entity_account_size = *serum_registry::accounts::entity::SIZE;
    let lamports = client
        .rpc()
        .get_minimum_balance_for_rent_exemption(entity_account_size as usize)?;

    let entity_address = entity_address_derived(client, &leader_kp.pubkey())?;
    let create_acc_instr = solana_sdk::system_instruction::create_account_with_seed(
        &client.payer().pubkey(), // From (signer).
        &entity_address,          // To.
        &leader_kp.pubkey(),      // Base (signer).
        entity_seed(),            // Seed.
        lamports,                 // Account start balance.
        entity_account_size,      // Acc size.
        &client.program(),        // Owner.
    );

    let accounts = [
        AccountMeta::new(entity_address, false),
        AccountMeta::new_readonly(leader_kp.pubkey(), true),
        AccountMeta::new_readonly(registrar, false),
        AccountMeta::new_readonly(solana_sdk::sysvar::rent::ID, false),
    ];
    let create_entity_instr = serum_registry::instruction::create_entity(
        *client.program(),
        &accounts,
        capabilities,
        stake_kind,
    );
    let instructions = [create_acc_instr, create_entity_instr];
    let signers = [leader_kp, client.payer()];
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
        .map(|sig| (sig, entity_address))
}

pub fn join_entity_derived(
    client: &InnerClient,
    registrar: Pubkey,
    entity: Pubkey,
    beneficiary: Pubkey,
    delegate: Pubkey,
    watchtower: Pubkey,
    watchtower_dest: Pubkey,
) -> Result<(Signature, Pubkey), InnerClientError> {
    let member_address = member_address_derived(client)?;

    let lamports = client
        .rpc()
        .get_minimum_balance_for_rent_exemption(*serum_registry::accounts::member::SIZE as usize)?;

    let create_acc_instr = solana_sdk::system_instruction::create_account_with_seed(
        &client.payer().pubkey(),
        &member_address,
        &client.payer().pubkey(),
        member_seed(),
        lamports,
        *serum_registry::accounts::member::SIZE,
        &client.program(),
    );

    let accounts = [
        AccountMeta::new(member_address, false),
        AccountMeta::new(entity, false),
        AccountMeta::new_readonly(registrar, false),
        AccountMeta::new_readonly(solana_sdk::sysvar::rent::ID, false),
    ];

    let member_instr = serum_registry::instruction::join_entity(
        *client.program(),
        &accounts,
        beneficiary,
        delegate,
        Watchtower::new(watchtower, watchtower_dest),
    );

    let instructions = [create_acc_instr, member_instr];
    let signers = [client.payer()];
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
        .map(|sig| (sig, member_address))
}

pub fn entity_address_derived(
    client: &InnerClient,
    leader: &Pubkey,
) -> Result<Pubkey, InnerClientError> {
    Pubkey::create_with_seed(leader, entity_seed(), &client.program())
        .map_err(|e| InnerClientError::RawError(e.to_string()))
}

pub fn entity_seed() -> &'static str {
    "srm:registry:entity"
}

pub fn member_address_derived(client: &InnerClient) -> Result<Pubkey, InnerClientError> {
    Pubkey::create_with_seed(&client.payer().pubkey(), member_seed(), &client.program())
        .map_err(|e| InnerClientError::RawError(e.to_string()))
}

pub fn member_seed() -> &'static str {
    "srm:registry:member"
}
