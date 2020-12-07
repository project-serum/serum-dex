use serum_rewards::accounts;
use serum_rewards::client::{Client as InnerClient, ClientError as InnerClientError};
use solana_client_gen::prelude::*;
use solana_client_gen::solana_sdk;
use solana_client_gen::solana_sdk::instruction::AccountMeta;
use solana_client_gen::solana_sdk::pubkey::Pubkey;

pub fn initialize(
    client: &InnerClient,
    registry_program_id: Pubkey,
    registrar: Pubkey,
    reward_mint: Pubkey,
    dex_program_id: Pubkey,
    authority: Pubkey,
) -> Result<(Signature, Pubkey, u8), InnerClientError> {
    let instance_kp = Keypair::generate(&mut OsRng);
    let (instance_vault_authority, nonce) =
        Pubkey::find_program_address(&[instance_kp.pubkey().as_ref()], client.program());

    let vault = serum_common::client::rpc::create_token_account(
        client.rpc(),
        &reward_mint,
        &instance_vault_authority,
        client.payer(),
    )
    .map_err(|e| InnerClientError::RawError(e.to_string()))?;

    let accounts = [
        AccountMeta::new(instance_kp.pubkey(), false),
        AccountMeta::new_readonly(vault.pubkey(), false),
        AccountMeta::new_readonly(registrar, false),
        AccountMeta::new_readonly(solana_sdk::sysvar::rent::id(), false),
    ];

    let create_instance_instr = {
        let lamports = client
            .rpc()
            .get_minimum_balance_for_rent_exemption(*accounts::instance::SIZE as usize)
            .map_err(InnerClientError::RpcError)?;
        system_instruction::create_account(
            &client.payer().pubkey(),
            &instance_kp.pubkey(),
            lamports,
            *accounts::instance::SIZE,
            client.program(),
        )
    };

    let initialize_instr = serum_rewards::instruction::initialize(
        *client.program(),
        &accounts,
        nonce,
        registry_program_id,
        dex_program_id,
        authority,
    );

    let tx = {
        let (recent_hash, _fee_calc) = client
            .rpc()
            .get_recent_blockhash()
            .map_err(|e| InnerClientError::RawError(e.to_string()))?;
        let signers = vec![client.payer(), &instance_kp];
        Transaction::new_signed_with_payer(
            &[create_instance_instr, initialize_instr],
            Some(&client.payer().pubkey()),
            &signers,
            recent_hash,
        )
    };
    client
        .rpc()
        .send_and_confirm_transaction_with_spinner_and_config(
            &tx,
            client.options().commitment,
            client.options().tx,
        )
        .map_err(InnerClientError::RpcError)
        .map(|tx| (tx, instance_kp.pubkey(), nonce))
}
