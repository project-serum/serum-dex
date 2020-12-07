use crate::InitializeResponse;
use serum_common::client::rpc;
use serum_common::pack::Pack;
use serum_lockup::accounts::{Safe, Whitelist};
use serum_lockup::client::{Client as InnerClient, ClientError as InnerClientError};
use solana_client_gen::prelude::*;
use solana_client_gen::solana_sdk;
use solana_client_gen::solana_sdk::instruction::AccountMeta;
use solana_client_gen::solana_sdk::pubkey::Pubkey;
use solana_client_gen::solana_sdk::system_instruction;
use spl_token::state::Account as TokenAccount;

pub fn create_all_accounts_and_initialize(
    client: &InnerClient,
    safe_authority: &Pubkey,
) -> Result<InitializeResponse, InnerClientError> {
    // Build the data dependent addresses.
    //
    // The safe instance requires a nonce for it's token vault, which
    // uses a program-derived address to "sign" transactions and
    // manage funds within the program.
    let safe_acc = Keypair::generate(&mut OsRng);

    // Now build the final transaction.
    let wl_kp = Keypair::generate(&mut OsRng);
    let instructions = {
        let create_safe_acc_instr = {
            let lamports = client
                .rpc()
                .get_minimum_balance_for_rent_exemption(Safe::default().size().unwrap() as usize)
                .map_err(InnerClientError::RpcError)?;
            system_instruction::create_account(
                &client.payer().pubkey(),
                &safe_acc.pubkey(),
                lamports,
                Safe::default().size().unwrap(),
                client.program(),
            )
        };
        let create_whitelist_acc_instr = {
            let lamports = client
                .rpc()
                .get_minimum_balance_for_rent_exemption(Whitelist::SIZE)
                .map_err(InnerClientError::RpcError)?;
            system_instruction::create_account(
                &client.payer().pubkey(),
                &wl_kp.pubkey(),
                lamports,
                Whitelist::SIZE as u64,
                client.program(),
            )
        };

        let accounts = [
            AccountMeta::new(safe_acc.pubkey(), false),
            AccountMeta::new(wl_kp.pubkey(), false),
            AccountMeta::new_readonly(solana_sdk::sysvar::rent::id(), false),
        ];

        let initialize_instr =
            serum_lockup::instruction::initialize(*client.program(), &accounts, *safe_authority);
        vec![
            create_safe_acc_instr,
            create_whitelist_acc_instr,
            initialize_instr,
        ]
    };

    let tx = {
        let (recent_hash, _fee_calc) = client
            .rpc()
            .get_recent_blockhash()
            .map_err(|e| InnerClientError::RawError(e.to_string()))?;
        let signers = vec![client.payer(), &safe_acc, &wl_kp];
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
        .map(|sig| InitializeResponse {
            tx: sig,
            safe: safe_acc.pubkey(),
            whitelist: wl_kp.pubkey(),
        })
}
pub fn create_vesting_account(
    client: &InnerClient,
    depositor: &Pubkey,
    depositor_owner: &Keypair,
    safe_acc: &Pubkey,
    vesting_acc_beneficiary: &Pubkey,
    end_ts: i64,
    period_count: u64,
    deposit_amount: u64,
) -> Result<(Signature, Keypair), InnerClientError> {
    let depositor_acc = rpc::get_token_account::<TokenAccount>(client.rpc(), depositor).unwrap();

    // The vesting account being created.
    let new_account = Keypair::generate(&mut OsRng);

    let (vault_authority, nonce) = Pubkey::find_program_address(
        &[safe_acc.as_ref(), vesting_acc_beneficiary.as_ref()],
        client.program(),
    );
    let vault = rpc::create_token_account(
        client.rpc(),
        &depositor_acc.mint,
        &vault_authority,
        client.payer(),
    )
    .unwrap()
    .pubkey();

    let deposit_accs = [
        AccountMeta::new(new_account.pubkey(), true),
        AccountMeta::new(*depositor, false),
        AccountMeta::new(depositor_owner.pubkey(), true),
        AccountMeta::new(vault, false),
        AccountMeta::new(*safe_acc, false),
        AccountMeta::new_readonly(spl_token::ID, false),
        AccountMeta::new_readonly(solana_sdk::sysvar::rent::ID, false),
        AccountMeta::new_readonly(solana_sdk::sysvar::clock::ID, false),
    ];
    let create_account_instr = {
        let lamports = client
            .rpc()
            .get_minimum_balance_for_rent_exemption(*serum_lockup::accounts::vesting::SIZE as usize)
            .map_err(InnerClientError::RpcError)?;
        system_instruction::create_account(
            &client.payer().pubkey(),
            &new_account.pubkey(),
            lamports,
            *serum_lockup::accounts::vesting::SIZE,
            client.program(),
        )
    };
    let create_vesting_instr = serum_lockup::instruction::create_vesting(
        *client.program(),
        &deposit_accs,
        *vesting_acc_beneficiary,
        end_ts,
        period_count,
        deposit_amount,
        nonce,
    );

    let instructions = [create_account_instr, create_vesting_instr];
    let tx = {
        let (recent_hash, _fee_calc) = client
            .rpc()
            .get_recent_blockhash()
            .map_err(|e| InnerClientError::RawError(e.to_string()))?;
        let signers = vec![client.payer(), depositor_owner, &new_account];
        Transaction::new_signed_with_payer(
            &instructions,
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
        .map(|sig| (sig, new_account))
}
