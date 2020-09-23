use crate::pack::Pack;
use anyhow::Result;
use rand::rngs::OsRng;
use solana_client::rpc_client::RpcClient;
use solana_client::rpc_config::RpcSendTransactionConfig;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, Signature, Signer};
use solana_sdk::transaction::Transaction;
use spl_token::instruction as token_instruction;
use spl_token::pack::Pack as TokenPack;

pub fn create_account_rent_exempt(
    client: &RpcClient,
    payer: &Keypair,
    data_size: usize,
    owner: &Pubkey,
) -> Result<Keypair> {
    let account = Keypair::generate(&mut OsRng);

    let signers = [payer, &account];

    let lamports = client.get_minimum_balance_for_rent_exemption(data_size)?;

    let create_account_instr = solana_sdk::system_instruction::create_account(
        &payer.pubkey(),
        &account.pubkey(),
        lamports,
        data_size as u64,
        owner,
    );

    let instructions = vec![create_account_instr];

    let (recent_hash, _fee_calc) = client.get_recent_blockhash()?;

    let txn = Transaction::new_signed_with_payer(
        &instructions,
        Some(&payer.pubkey()),
        &signers,
        recent_hash,
    );

    send_txn(client, &txn, false)?;
    Ok(account)
}

pub fn create_token_account(
    client: &RpcClient,
    mint_pubkey: &Pubkey,
    owner_pubkey: &Pubkey,
    payer: &Keypair,
) -> Result<Keypair> {
    let spl_account = Keypair::generate(&mut OsRng);
    let signers = vec![payer, &spl_account];

    let lamports = client.get_minimum_balance_for_rent_exemption(spl_token::state::Account::LEN)?;

    let create_account_instr = solana_sdk::system_instruction::create_account(
        &payer.pubkey(),
        &spl_account.pubkey(),
        lamports,
        spl_token::state::Account::LEN as u64,
        &spl_token::ID,
    );

    let init_account_instr = token_instruction::initialize_account(
        &spl_token::ID,
        &spl_account.pubkey(),
        &mint_pubkey,
        &owner_pubkey,
    )?;

    let instructions = vec![create_account_instr, init_account_instr];

    let (recent_hash, _fee_calc) = client.get_recent_blockhash()?;

    let txn = Transaction::new_signed_with_payer(
        &instructions,
        Some(&payer.pubkey()),
        &signers,
        recent_hash,
    );

    println!("Creating account: {} ...", spl_account.pubkey());
    send_txn(client, &txn, false)?;
    Ok(spl_account)
}

pub fn create_and_init_mint(
    client: &RpcClient,
    payer_keypair: &Keypair,
    mint_keypair: &Keypair,
    owner_pubkey: &Pubkey,
    decimals: u8,
) -> Result<Signature> {
    let signers = vec![payer_keypair, mint_keypair];

    let lamports = client.get_minimum_balance_for_rent_exemption(spl_token::state::Mint::LEN)?;

    let create_mint_account_instruction = solana_sdk::system_instruction::create_account(
        &payer_keypair.pubkey(),
        &mint_keypair.pubkey(),
        lamports,
        spl_token::state::Mint::LEN as u64,
        &spl_token::ID,
    );
    let initialize_mint_instruction = token_instruction::initialize_mint(
        &spl_token::ID,
        &mint_keypair.pubkey(),
        owner_pubkey,
        None,
        decimals,
    )?;
    let instructions = vec![create_mint_account_instruction, initialize_mint_instruction];

    let (recent_hash, _fee_calc) = client.get_recent_blockhash()?;
    let txn = Transaction::new_signed_with_payer(
        &instructions,
        Some(&payer_keypair.pubkey()),
        &signers,
        recent_hash,
    );

    send_txn(client, &txn, false)
}

pub fn mint_to_new_account(
    client: &RpcClient,
    payer: &Keypair,
    minting_key: &Keypair,
    mint: &Pubkey,
    quantity: u64,
) -> Result<Keypair> {
    let recip_keypair = Keypair::generate(&mut OsRng);

    let lamports = client.get_minimum_balance_for_rent_exemption(spl_token::state::Account::LEN)?;

    let signers = vec![payer, minting_key, &recip_keypair];

    let create_recip_instr = solana_sdk::system_instruction::create_account(
        &payer.pubkey(),
        &recip_keypair.pubkey(),
        lamports,
        spl_token::state::Account::LEN as u64,
        &spl_token::ID,
    );

    let init_recip_instr = token_instruction::initialize_account(
        &spl_token::ID,
        &recip_keypair.pubkey(),
        mint,
        &payer.pubkey(),
    )?;

    let mint_tokens_instr = token_instruction::mint_to(
        &spl_token::ID,
        mint,
        &recip_keypair.pubkey(),
        &minting_key.pubkey(),
        &[],
        quantity,
    )?;

    let instructions = vec![create_recip_instr, init_recip_instr, mint_tokens_instr];

    let (recent_hash, _fee_calc) = client.get_recent_blockhash()?;
    let txn = Transaction::new_signed_with_payer(
        &instructions,
        Some(&payer.pubkey()),
        &signers,
        recent_hash,
    );

    send_txn(client, &txn, false)?;
    Ok(recip_keypair)
}

pub fn send_txn(client: &RpcClient, txn: &Transaction, _simulate: bool) -> Result<Signature> {
    Ok(client.send_and_confirm_transaction_with_spinner_and_config(
        txn,
        CommitmentConfig::single(),
        RpcSendTransactionConfig {
            skip_preflight: true,
            preflight_commitment: None,
        },
    )?)
}

pub fn account_token_unpacked<T: TokenPack>(client: &RpcClient, addr: &Pubkey) -> T {
    let account = client
        .get_account_with_commitment(addr, CommitmentConfig::recent())
        .unwrap()
        .value
        .unwrap();
    T::unpack_from_slice(&account.data).unwrap()
}

pub fn account_unpacked<'a, T: Pack<'a>>(client: &RpcClient, addr: &Pubkey) -> T {
    let account = client
        .get_account_with_commitment(addr, CommitmentConfig::recent())
        .unwrap()
        .value
        .unwrap();
    T::unpack(&account.data).unwrap()
}
