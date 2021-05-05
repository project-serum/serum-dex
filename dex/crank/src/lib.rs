#![deny(safe_packed_borrows)]
#![allow(dead_code)]

use std::borrow::Cow;
use std::cmp::{max, min};
use std::collections::BTreeSet;
use std::convert::identity;
use std::mem::size_of;
use std::num::NonZeroU64;
use std::sync::{Arc, Mutex};
use std::{thread, time};

use anyhow::{format_err, Result};
use clap::Clap;
use debug_print::debug_println;
use log::{error, info};
use rand::rngs::OsRng;
use safe_transmute::{
    guard::SingleManyGuard,
    to_bytes::{transmute_one_to_bytes, transmute_to_bytes},
    transmute_many, transmute_many_pedantic, transmute_one_pedantic,
};
use sloggers::file::FileLoggerBuilder;
use sloggers::types::Severity;
use sloggers::Build;
use solana_client::rpc_client::RpcClient;
use solana_client::rpc_config::RpcSendTransactionConfig;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::program_pack::Pack;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Signature;
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk::transaction::Transaction;
use spl_token::instruction as token_instruction;
use warp::Filter;

use serum_common::client::rpc::{
    create_and_init_mint, create_token_account, mint_to_new_account, send_txn, simulate_transaction,
};
use serum_common::client::Cluster;
use serum_context::Context;
use serum_dex::instruction::{
    cancel_order_by_client_order_id as cancel_order_by_client_order_id_ix,
    close_open_orders as close_open_orders_ix, init_open_orders as init_open_orders_ix,
    MarketInstruction, NewOrderInstructionV3, SelfTradeBehavior,
};
use serum_dex::matching::{OrderType, Side};
use serum_dex::state::gen_vault_signer_key;
use serum_dex::state::Event;
use serum_dex::state::EventQueueHeader;
use serum_dex::state::MarketState;
use serum_dex::state::QueueHeader;
use serum_dex::state::Request;
use serum_dex::state::RequestQueueHeader;

pub fn with_logging<F: FnOnce()>(_to: &str, fnc: F) {
    fnc();
}

fn read_keypair_file(s: &str) -> Result<Keypair> {
    solana_sdk::signature::read_keypair_file(s)
        .map_err(|_| format_err!("failed to read keypair from {}", s))
}

#[derive(Clap, Debug)]
pub struct Opts {
    #[clap(default_value = "mainnet")]
    pub cluster: Cluster,
    #[clap(subcommand)]
    pub command: Command,
}

impl Opts {
    fn client(&self) -> RpcClient {
        RpcClient::new(self.cluster.url().to_string())
    }
}

#[derive(Clap, Debug)]
pub enum Command {
    Genesis {
        #[clap(long, short)]
        payer: String,

        #[clap(long, short)]
        mint: String,

        #[clap(long, short)]
        owner_pubkey: Pubkey,

        #[clap(long, short)]
        decimals: u8,
    },
    Mint {
        #[clap(long, short)]
        payer: String,

        #[clap(long, short)]
        signer: String,

        #[clap(long, short)]
        mint_pubkey: Pubkey,

        #[clap(long, short)]
        recipient: Option<Pubkey>,

        #[clap(long, short)]
        quantity: u64,
    },
    CreateAccount {
        mint_pubkey: Pubkey,
        owner_pubkey: Pubkey,
        payer: String,
    },
    ConsumeEvents {
        #[clap(long, short)]
        dex_program_id: Pubkey,
        #[clap(long)]
        payer: String,
        #[clap(long, short)]
        market: Pubkey,
        #[clap(long, short)]
        coin_wallet: Pubkey,
        #[clap(long, short)]
        pc_wallet: Pubkey,
        #[clap(long, short)]
        num_workers: usize,
        #[clap(long, short)]
        events_per_worker: usize,
        #[clap(long)]
        num_accounts: Option<usize>,
        #[clap(long)]
        log_directory: String,
        #[clap(long)]
        max_q_length: Option<u64>,
        #[clap(long)]
        max_wait_for_events_delay: Option<u64>,
    },
    MatchOrders {
        #[clap(long, short)]
        dex_program_id: Pubkey,

        #[clap(long)]
        payer: String,

        #[clap(long, short)]
        market: Pubkey,

        #[clap(long, short)]
        coin_wallet: Pubkey,

        #[clap(long, short)]
        pc_wallet: Pubkey,
    },
    MonitorQueue {
        #[clap(long, short)]
        dex_program_id: Pubkey,

        #[clap(long, short)]
        market: Pubkey,

        #[clap(long)]
        port: u16,
    },
    PrintEventQueue {
        dex_program_id: Pubkey,
        market: Pubkey,
    },
    WholeShebang {
        payer: String,
        dex_program_id: Pubkey,
    },
    SettleFunds {
        payer: String,
        dex_program_id: Pubkey,
        market: Pubkey,
        orders: Pubkey,
        coin_wallet: Pubkey,
        pc_wallet: Pubkey,
        #[clap(long, short)]
        signer: Option<String>,
    },
    ListMarket {
        payer: String,
        dex_program_id: Pubkey,
        #[clap(long, short)]
        coin_mint: Pubkey,
        #[clap(long, short)]
        pc_mint: Pubkey,
        #[clap(long)]
        coin_lot_size: Option<u64>,
        #[clap(long)]
        pc_lot_size: Option<u64>,
    },
    InitializeTokenAccount {
        mint: Pubkey,
        owner_account: String,
    },
}

pub fn start(ctx: Option<Context>, opts: Opts) -> Result<()> {
    let client = opts.client();

    match opts.command {
        Command::Genesis {
            payer,
            mint,
            owner_pubkey,
            decimals,
        } => {
            let payer = read_keypair_file(&payer)?;
            let mint = read_keypair_file(&mint)?;
            create_and_init_mint(&client, &payer, &mint, &owner_pubkey, decimals)?;
        }
        Command::Mint {
            payer,
            signer,
            mint_pubkey,
            recipient,
            quantity,
        } => {
            let payer = read_keypair_file(&payer)?;
            let minter = read_keypair_file(&signer)?;
            match recipient.as_ref() {
                Some(recipient) => {
                    mint_to_existing_account(
                        &client,
                        &payer,
                        &minter,
                        &mint_pubkey,
                        recipient,
                        quantity,
                    )?;
                }
                None => {
                    mint_to_new_account(&client, &payer, &minter, &mint_pubkey, quantity)?;
                }
            };
        }
        Command::CreateAccount { .. } => unimplemented!(),
        Command::MatchOrders {
            ref dex_program_id,
            ref payer,
            ref market,
            ref coin_wallet,
            ref pc_wallet,
        } => {
            let payer = read_keypair_file(&payer)?;

            debug_println!("Getting market keys ...");
            let market_keys = get_keys_for_market(&client, dex_program_id, &market)?;
            debug_println!("{:#?}", market_keys);
            match_orders(
                &client,
                dex_program_id,
                &payer,
                &market_keys,
                coin_wallet,
                pc_wallet,
            )?;
        }
        Command::ConsumeEvents {
            ref dex_program_id,
            ref payer,
            ref market,
            ref coin_wallet,
            ref pc_wallet,
            num_workers,
            events_per_worker,
            ref num_accounts,
            ref log_directory,
            ref max_q_length,
            ref max_wait_for_events_delay,
        } => {
            init_logger(log_directory);
            consume_events_loop(
                &opts,
                &dex_program_id,
                &payer,
                &market,
                &coin_wallet,
                &pc_wallet,
                num_workers,
                events_per_worker,
                num_accounts.unwrap_or(32),
                max_q_length.unwrap_or(1),
                max_wait_for_events_delay.unwrap_or(60),
            )?;
        }
        Command::MonitorQueue {
            dex_program_id,
            market,
            port,
        } => {
            let client = opts.client();
            let mut runtime = tokio::runtime::Builder::new()
                .basic_scheduler()
                .build()
                .unwrap();
            runtime
                .block_on(read_queue_length_loop(client, dex_program_id, market, port))
                .unwrap();
        }
        Command::PrintEventQueue {
            ref dex_program_id,
            ref market,
        } => {
            let market_keys = get_keys_for_market(&client, dex_program_id, &market)?;
            let event_q_data = client.get_account_data(&market_keys.event_q)?;
            let inner: Cow<[u64]> = remove_dex_account_padding(&event_q_data)?;
            let (header, events_seg0, events_seg1) = parse_event_queue(&inner)?;
            debug_println!("Header:\n{:#x?}", header);
            debug_println!("Seg0:\n{:#x?}", events_seg0);
            debug_println!("Seg1:\n{:#x?}", events_seg1);
        }
        Command::WholeShebang {
            ref dex_program_id,
            ref payer,
        } => {
            let payer = read_keypair_file(payer)?;
            whole_shebang(&client, dex_program_id, &payer)?;
        }
        Command::SettleFunds {
            ref payer,
            ref dex_program_id,
            ref market,
            ref orders,
            ref coin_wallet,
            ref pc_wallet,
            ref signer,
        } => {
            let payer = read_keypair_file(payer)?;
            let signer = signer.as_ref().map(|s| read_keypair_file(&s)).transpose()?;
            let market_keys = get_keys_for_market(&client, dex_program_id, &market)?;
            settle_funds(
                &client,
                dex_program_id,
                &payer,
                &market_keys,
                signer.as_ref(),
                orders,
                coin_wallet,
                pc_wallet,
            )?;
        }
        Command::ListMarket {
            ref payer,
            ref dex_program_id,
            ref coin_mint,
            ref pc_mint,
            coin_lot_size,
            pc_lot_size,
        } => {
            let payer = read_keypair_file(payer)?;
            let market_keys = list_market(
                &client,
                dex_program_id,
                &payer,
                coin_mint,
                pc_mint,
                coin_lot_size.unwrap_or(1_000_000),
                pc_lot_size.unwrap_or(10_000),
            )?;
            println!("Listed market: {:#?}", market_keys);
        }
        Command::InitializeTokenAccount {
            ref mint,
            ref owner_account,
        } => {
            let owner = read_keypair_file(owner_account)?;
            let initialized_account = initialize_token_account(&client, mint, &owner)?;
            debug_println!("Initialized account: {}", initialized_account.pubkey());
        }
    }
    Ok(())
}

#[derive(Debug)]
pub struct MarketPubkeys {
    pub market: Box<Pubkey>,
    pub req_q: Box<Pubkey>,
    pub event_q: Box<Pubkey>,
    pub bids: Box<Pubkey>,
    pub asks: Box<Pubkey>,
    pub coin_vault: Box<Pubkey>,
    pub pc_vault: Box<Pubkey>,
    pub vault_signer_key: Box<Pubkey>,
}

#[cfg(target_endian = "little")]
fn remove_dex_account_padding<'a>(data: &'a [u8]) -> Result<Cow<'a, [u64]>> {
    use serum_dex::state::{ACCOUNT_HEAD_PADDING, ACCOUNT_TAIL_PADDING};
    let head = &data[..ACCOUNT_HEAD_PADDING.len()];
    if data.len() < ACCOUNT_HEAD_PADDING.len() + ACCOUNT_TAIL_PADDING.len() {
        return Err(format_err!(
            "dex account length {} is too small to contain valid padding",
            data.len()
        ));
    }
    if head != ACCOUNT_HEAD_PADDING {
        return Err(format_err!("dex account head padding mismatch"));
    }
    let tail = &data[data.len() - ACCOUNT_TAIL_PADDING.len()..];
    if tail != ACCOUNT_TAIL_PADDING {
        return Err(format_err!("dex account tail padding mismatch"));
    }
    let inner_data_range = ACCOUNT_HEAD_PADDING.len()..(data.len() - ACCOUNT_TAIL_PADDING.len());
    let inner: &'a [u8] = &data[inner_data_range];
    let words: Cow<'a, [u64]> = match transmute_many_pedantic::<u64>(inner) {
        Ok(word_slice) => Cow::Borrowed(word_slice),
        Err(transmute_error) => {
            let word_vec = transmute_error.copy().map_err(|e| e.without_src())?;
            Cow::Owned(word_vec)
        }
    };
    Ok(words)
}

#[cfg(target_endian = "little")]
fn get_keys_for_market<'a>(
    client: &'a RpcClient,
    program_id: &'a Pubkey,
    market: &'a Pubkey,
) -> Result<MarketPubkeys> {
    let account_data: Vec<u8> = client.get_account_data(&market)?;
    let words: Cow<[u64]> = remove_dex_account_padding(&account_data)?;
    let market_state: MarketState =
        transmute_one_pedantic::<MarketState>(transmute_to_bytes(&words))
            .map_err(|e| e.without_src())?;
    market_state.check_flags()?;
    let vault_signer_key =
        gen_vault_signer_key(market_state.vault_signer_nonce, market, program_id)?;
    assert_eq!(
        transmute_to_bytes(&identity(market_state.own_address)),
        market.as_ref()
    );
    Ok(MarketPubkeys {
        market: Box::new(*market),
        req_q: Box::new(Pubkey::new(transmute_one_to_bytes(&identity(
            market_state.req_q,
        )))),
        event_q: Box::new(Pubkey::new(transmute_one_to_bytes(&identity(
            market_state.event_q,
        )))),
        bids: Box::new(Pubkey::new(transmute_one_to_bytes(&identity(
            market_state.bids,
        )))),
        asks: Box::new(Pubkey::new(transmute_one_to_bytes(&identity(
            market_state.asks,
        )))),
        coin_vault: Box::new(Pubkey::new(transmute_one_to_bytes(&identity(
            market_state.coin_vault,
        )))),
        pc_vault: Box::new(Pubkey::new(transmute_one_to_bytes(&identity(
            market_state.pc_vault,
        )))),
        vault_signer_key: Box::new(vault_signer_key),
    })
}

fn parse_event_queue(data_words: &[u64]) -> Result<(EventQueueHeader, &[Event], &[Event])> {
    let (header_words, event_words) = data_words.split_at(size_of::<EventQueueHeader>() >> 3);
    let header: EventQueueHeader =
        transmute_one_pedantic(transmute_to_bytes(header_words)).map_err(|e| e.without_src())?;
    let events: &[Event] = transmute_many::<_, SingleManyGuard>(transmute_to_bytes(event_words))
        .map_err(|e| e.without_src())?;
    let (tail_seg, head_seg) = events.split_at(header.head() as usize);
    let head_len = head_seg.len().min(header.count() as usize);
    let tail_len = header.count() as usize - head_len;
    Ok((header, &head_seg[..head_len], &tail_seg[..tail_len]))
}

fn parse_req_queue(data_words: &[u64]) -> Result<(RequestQueueHeader, &[Request], &[Request])> {
    let (header_words, request_words) = data_words.split_at(size_of::<RequestQueueHeader>() >> 3);
    let header: RequestQueueHeader =
        transmute_one_pedantic(transmute_to_bytes(header_words)).map_err(|e| e.without_src())?;
    let request: &[Request] =
        transmute_many::<_, SingleManyGuard>(transmute_to_bytes(request_words))
            .map_err(|e| e.without_src())?;
    let (tail_seg, head_seg) = request.split_at(header.head() as usize);
    let head_len = head_seg.len().min(header.count() as usize);
    let tail_len = header.count() as usize - head_len;
    Ok((header, &head_seg[..head_len], &tail_seg[..tail_len]))
}

fn hash_accounts(val: &[u64; 4]) -> u64 {
    val.iter().fold(0, |a, b| b.wrapping_add(a))
}

fn init_logger(log_directory: &str) {
    let path = std::path::Path::new(log_directory);
    let parent = path.parent().unwrap();
    std::fs::create_dir_all(parent).unwrap();
    let mut builder = FileLoggerBuilder::new(log_directory);
    builder.level(Severity::Info).rotate_size(8 * 1024 * 1024);
    let log = builder.build().unwrap();
    let _guard = slog_scope::set_global_logger(log);
    _guard.cancel_reset();
    slog_stdlog::init().unwrap();
}

fn consume_events_loop(
    opts: &Opts,
    program_id: &Pubkey,
    payer_path: &String,
    market: &Pubkey,
    coin_wallet: &Pubkey,
    pc_wallet: &Pubkey,
    num_workers: usize,
    events_per_worker: usize,
    num_accounts: usize,
    max_q_length: u64,
    max_wait_for_events_delay: u64,
) -> Result<()> {
    info!("Getting market keys ...");
    let client = opts.client();
    let market_keys = get_keys_for_market(&client, &program_id, &market)?;
    info!("{:#?}", market_keys);
    let pool = threadpool::ThreadPool::new(num_workers);
    let max_slot_height_mutex = Arc::new(Mutex::new(0_u64));
    let mut last_cranked_at = std::time::Instant::now()
        .checked_sub(std::time::Duration::from_secs(max_wait_for_events_delay))
        .unwrap_or(std::time::Instant::now());

    loop {
        thread::sleep(time::Duration::from_millis(1000));

        let loop_start = std::time::Instant::now();
        let start_time = std::time::Instant::now();
        let event_q_value_and_context =
            client.get_account_with_commitment(&market_keys.event_q, CommitmentConfig::recent())?;
        let event_q_slot = event_q_value_and_context.context.slot;
        let max_slot_height = max_slot_height_mutex.lock().unwrap();
        if event_q_slot <= *max_slot_height {
            info!(
                "Skipping crank. Already cranked for slot. Event queue slot: {}, Max seen slot: {}",
                event_q_slot, max_slot_height
            );
            continue;
        }
        drop(max_slot_height);
        let event_q_data = event_q_value_and_context
            .value
            .ok_or(format_err!("Failed to retrieve account"))?
            .data;
        let req_q_data = client
            .get_account_with_commitment(&market_keys.req_q, CommitmentConfig::recent())?
            .value
            .ok_or(format_err!("Failed to retrieve account"))?
            .data;
        let inner: Cow<[u64]> = remove_dex_account_padding(&event_q_data)?;
        let (_header, seg0, seg1) = parse_event_queue(&inner)?;
        let req_inner: Cow<[u64]> = remove_dex_account_padding(&req_q_data)?;
        let (_req_header, req_seg0, req_seg1) = parse_event_queue(&req_inner)?;
        let event_q_len = seg0.len() + seg1.len();
        let req_q_len = req_seg0.len() + req_seg1.len();
        info!(
            "Size of request queue is {}, market {}, coin {}, pc {}",
            req_q_len, market, coin_wallet, pc_wallet
        );

        if event_q_len == 0 {
            continue;
        } else if std::time::Duration::from_secs(max_wait_for_events_delay)
            .gt(&last_cranked_at.elapsed())
            && (event_q_len as u64) < max_q_length
        {
            info!(
                "Skipping crank. Last cranked {} seconds ago and queue only has {} events. \
                Event queue slot: {}",
                last_cranked_at.elapsed().as_secs(),
                event_q_len,
                event_q_slot
            );
            continue;
        } else {
            info!(
                "Total event queue length: {}, market {}, coin {}, pc {}",
                event_q_len, market, coin_wallet, pc_wallet
            );
            let accounts = seg0.iter().chain(seg1.iter()).map(|event| event.owner);
            let mut used_accounts = BTreeSet::new();
            for account in accounts {
                used_accounts.insert(account);
                if used_accounts.len() >= num_accounts {
                    break;
                }
            }
            let orders_accounts: Vec<_> = used_accounts.into_iter().collect();
            info!(
                "Number of unique order accounts: {}, market {}, coin {}, pc {}",
                orders_accounts.len(),
                market,
                coin_wallet,
                pc_wallet
            );
            info!(
                "First 5 accounts: {:?}",
                orders_accounts
                    .iter()
                    .take(5)
                    .map(hash_accounts)
                    .collect::<Vec::<_>>()
            );

            let mut account_metas = Vec::with_capacity(orders_accounts.len() + 4);
            for pubkey_words in orders_accounts {
                let pubkey = Pubkey::new(transmute_to_bytes(&pubkey_words));
                account_metas.push(AccountMeta::new(pubkey, false));
            }
            for pubkey in [
                &market_keys.market,
                &market_keys.event_q,
                coin_wallet,
                pc_wallet,
            ]
            .iter()
            {
                account_metas.push(AccountMeta::new(**pubkey, false));
            }
            debug_println!("Number of workers: {}", num_workers);
            let end_time = std::time::Instant::now();
            info!(
                "Fetching {} events from the queue took {}",
                event_q_len,
                end_time.duration_since(start_time).as_millis()
            );
            for thread_num in 0..min(num_workers, 2 * event_q_len / events_per_worker + 1) {
                let payer = read_keypair_file(&payer_path)?;
                let program_id = program_id.clone();
                let client = opts.client();
                let account_metas = account_metas.clone();
                let event_q = *market_keys.event_q;
                let max_slot_height_mutex_clone = Arc::clone(&max_slot_height_mutex);
                pool.execute(move || {
                    consume_events_wrapper(
                        &client,
                        &program_id,
                        &payer,
                        account_metas,
                        thread_num,
                        events_per_worker,
                        event_q,
                        max_slot_height_mutex_clone,
                        event_q_slot,
                    )
                });
            }
            pool.join();
            last_cranked_at = std::time::Instant::now();
            info!(
                "Total loop time took {}",
                last_cranked_at.duration_since(loop_start).as_millis()
            );
        }
    }
}

fn consume_events_wrapper(
    client: &RpcClient,
    program_id: &Pubkey,
    payer: &Keypair,
    account_metas: Vec<AccountMeta>,
    thread_num: usize,
    to_consume: usize,
    event_q: Pubkey,
    max_slot_height_mutex: Arc<Mutex<u64>>,
    slot: u64,
) {
    let start = std::time::Instant::now();
    let result = consume_events_once(
        &client,
        program_id,
        &payer,
        account_metas,
        to_consume,
        thread_num,
        event_q,
    );
    match result {
        Ok(signature) => {
            info!(
                "[thread {}] Successfully consumed events after {:?}: {}.",
                thread_num,
                start.elapsed(),
                signature
            );
            let mut max_slot_height = max_slot_height_mutex.lock().unwrap();
            *max_slot_height = max(slot, *max_slot_height);
        }
        Err(err) => {
            error!("[thread {}] Received error: {:?}", thread_num, err);
        }
    };
}

fn consume_events_once(
    client: &RpcClient,
    program_id: &Pubkey,
    payer: &Keypair,
    account_metas: Vec<AccountMeta>,
    to_consume: usize,
    _thread_number: usize,
    event_q: Pubkey,
) -> Result<Signature> {
    let _start = std::time::Instant::now();
    let instruction_data: Vec<u8> = MarketInstruction::ConsumeEvents(to_consume as u16).pack();
    let instruction = Instruction {
        program_id: *program_id,
        accounts: account_metas,
        data: instruction_data,
    };
    let random_instruction = solana_sdk::system_instruction::transfer(
        &payer.pubkey(),
        &payer.pubkey(),
        rand::random::<u64>() % 10000 + 1,
    );
    let (recent_hash, _fee_calc) = client.get_recent_blockhash()?;
    let txn = Transaction::new_signed_with_payer(
        &[instruction, random_instruction],
        Some(&payer.pubkey()),
        &[payer],
        recent_hash,
    );

    info!("Consuming events ...");
    let signature = client.send_transaction_with_config(
        &txn,
        RpcSendTransactionConfig {
            skip_preflight: true,
            ..RpcSendTransactionConfig::default()
        },
    )?;
    Ok(signature)
}

#[cfg(target_endian = "little")]
fn consume_events(
    client: &RpcClient,
    program_id: &Pubkey,
    payer: &Keypair,
    state: &MarketPubkeys,
    coin_wallet: &Pubkey,
    pc_wallet: &Pubkey,
) -> Result<()> {
    let instruction = {
        let i = consume_events_instruction(client, program_id, state, coin_wallet, pc_wallet)?;
        match i {
            None => return Ok(()),
            Some(i) => i,
        }
    };
    let (recent_hash, _fee_calc) = client.get_recent_blockhash()?;
    info!("Consuming events ...");
    let txn = Transaction::new_signed_with_payer(
        std::slice::from_ref(&instruction),
        Some(&payer.pubkey()),
        &[payer],
        recent_hash,
    );
    info!("Consuming events ...");
    send_txn(client, &txn, false)?;
    Ok(())
}

pub fn consume_events_instruction(
    client: &RpcClient,
    program_id: &Pubkey,
    state: &MarketPubkeys,
    coin_wallet: &Pubkey,
    pc_wallet: &Pubkey,
) -> Result<Option<Instruction>> {
    let event_q_data = client.get_account_data(&state.event_q)?;
    let inner: Cow<[u64]> = remove_dex_account_padding(&event_q_data)?;
    let (_header, seg0, seg1) = parse_event_queue(&inner)?;

    if seg0.len() + seg1.len() == 0 {
        info!("Total event queue length: 0, returning early");
        return Ok(None);
    } else {
        info!("Total event queue length: {}", seg0.len() + seg1.len());
    }
    let accounts = seg0.iter().chain(seg1.iter()).map(|event| event.owner);
    let mut orders_accounts: Vec<_> = accounts.collect();
    orders_accounts.sort_unstable();
    orders_accounts.dedup();
    // todo: Shuffle the accounts before truncating, to avoid favoring low sort order accounts
    orders_accounts.truncate(32);
    info!("Number of unique order accounts: {}", orders_accounts.len());

    let mut account_metas = Vec::with_capacity(orders_accounts.len() + 4);
    for pubkey_words in orders_accounts {
        let pubkey = Pubkey::new(transmute_to_bytes(&pubkey_words));
        account_metas.push(AccountMeta::new(pubkey, false));
    }
    for pubkey in [&state.market, &state.event_q, coin_wallet, pc_wallet].iter() {
        account_metas.push(AccountMeta::new(**pubkey, false));
    }

    let instruction_data: Vec<u8> =
        MarketInstruction::ConsumeEvents(account_metas.len() as u16).pack();

    let instruction = Instruction {
        program_id: *program_id,
        accounts: account_metas,
        data: instruction_data,
    };

    Ok(Some(instruction))
}

fn whole_shebang(client: &RpcClient, program_id: &Pubkey, payer: &Keypair) -> Result<()> {
    let coin_mint = Keypair::generate(&mut OsRng);
    debug_println!("Coin mint: {}", coin_mint.pubkey());
    create_and_init_mint(client, payer, &coin_mint, &payer.pubkey(), 3)?;

    let pc_mint = Keypair::generate(&mut OsRng);
    debug_println!("Pc mint: {}", pc_mint.pubkey());
    create_and_init_mint(client, payer, &pc_mint, &payer.pubkey(), 3)?;

    let market_keys = list_market(
        client,
        program_id,
        payer,
        &coin_mint.pubkey(),
        &pc_mint.pubkey(),
        1_000_000,
        10_000,
    )?;
    debug_println!("Market keys: {:#?}", market_keys);

    debug_println!("Minting coin...");
    let coin_wallet = mint_to_new_account(
        client,
        payer,
        payer,
        &coin_mint.pubkey(),
        1_000_000_000_000_000,
    )?;
    debug_println!("Minted {}", coin_wallet.pubkey());

    debug_println!("Minting price currency...");
    let pc_wallet = mint_to_new_account(
        client,
        payer,
        payer,
        &pc_mint.pubkey(),
        1_000_000_000_000_000,
    )?;
    debug_println!("Minted {}", pc_wallet.pubkey());

    let mut orders = None;

    debug_println!("Initializing open orders");
    init_open_orders(client, program_id, payer, &market_keys, &mut orders)?;

    debug_println!("Placing bid...");
    place_order(
        client,
        program_id,
        payer,
        &pc_wallet.pubkey(),
        &market_keys,
        &mut orders,
        NewOrderInstructionV3 {
            side: Side::Bid,
            limit_price: NonZeroU64::new(500).unwrap(),
            max_coin_qty: NonZeroU64::new(1_000).unwrap(),
            max_native_pc_qty_including_fees: NonZeroU64::new(500_000).unwrap(),
            order_type: OrderType::Limit,
            client_order_id: 019269,
            self_trade_behavior: SelfTradeBehavior::DecrementTake,
            limit: std::u16::MAX,
        },
    )?;

    debug_println!("Bid account: {}", orders.unwrap());

    debug_println!("Placing offer...");
    let mut orders = None;
    place_order(
        client,
        program_id,
        payer,
        &coin_wallet.pubkey(),
        &market_keys,
        &mut orders,
        NewOrderInstructionV3 {
            side: Side::Ask,
            limit_price: NonZeroU64::new(499).unwrap(),
            max_coin_qty: NonZeroU64::new(1_000).unwrap(),
            max_native_pc_qty_including_fees: NonZeroU64::new(std::u64::MAX).unwrap(),
            order_type: OrderType::Limit,
            limit: std::u16::MAX,
            self_trade_behavior: SelfTradeBehavior::DecrementTake,
            client_order_id: 985982,
        },
    )?;

    // Cancel the open order so that we can close it later.
    cancel_order_by_client_order_id(
        client,
        program_id,
        payer,
        &market_keys,
        &orders.unwrap(),
        985982,
    )?;

    debug_println!("Ask account: {}", orders.unwrap());

    debug_println!("Consuming events in 15s ...");
    std::thread::sleep(std::time::Duration::new(15, 0));
    consume_events(
        client,
        program_id,
        payer,
        &market_keys,
        &coin_wallet.pubkey(),
        &pc_wallet.pubkey(),
    )?;
    settle_funds(
        client,
        program_id,
        payer,
        &market_keys,
        Some(payer),
        &orders.unwrap(),
        &coin_wallet.pubkey(),
        &pc_wallet.pubkey(),
    )?;
    close_open_orders(
        client,
        program_id,
        payer,
        &market_keys,
        orders.as_ref().unwrap(),
    )?;
    Ok(())
}

pub fn cancel_order_by_client_order_id(
    client: &RpcClient,
    program_id: &Pubkey,
    owner: &Keypair,
    state: &MarketPubkeys,
    orders: &Pubkey,
    client_order_id: u64,
) -> Result<()> {
    let ixs = &[cancel_order_by_client_order_id_ix(
        program_id,
        &state.market,
        &state.bids,
        &state.asks,
        orders,
        &owner.pubkey(),
        &state.event_q,
        client_order_id,
    )?];
    let (recent_hash, _fee_calc) = client.get_recent_blockhash()?;
    let txn = Transaction::new_signed_with_payer(ixs, Some(&owner.pubkey()), &[owner], recent_hash);

    debug_println!("Canceling order by client order id instruction ...");
    let result = simulate_transaction(client, &txn, true, CommitmentConfig::confirmed())?;
    if let Some(e) = result.value.err {
        debug_println!("{:#?}", result.value.logs);
        return Err(format_err!("simulate_transaction error: {:?}", e));
    }

    send_txn(client, &txn, false)?;
    Ok(())
}

pub fn close_open_orders(
    client: &RpcClient,
    program_id: &Pubkey,
    owner: &Keypair,
    state: &MarketPubkeys,
    orders: &Pubkey,
) -> Result<()> {
    debug_println!("Closing open orders...");
    let ixs = &[close_open_orders_ix(
        program_id,
        orders,
        &owner.pubkey(),
        &owner.pubkey(),
        &state.market,
    )?];
    let (recent_hash, _fee_calc) = client.get_recent_blockhash()?;
    let txn = Transaction::new_signed_with_payer(ixs, Some(&owner.pubkey()), &[owner], recent_hash);

    debug_println!("Simulating close open orders instruction ...");
    let result = simulate_transaction(client, &txn, true, CommitmentConfig::confirmed())?;
    if let Some(e) = result.value.err {
        debug_println!("{:#?}", result.value.logs);
        return Err(format_err!("simulate_transaction error: {:?}", e));
    }

    send_txn(client, &txn, false)?;
    Ok(())
}

pub fn init_open_orders(
    client: &RpcClient,
    program_id: &Pubkey,
    owner: &Keypair,
    state: &MarketPubkeys,
    orders: &mut Option<Pubkey>,
) -> Result<()> {
    let mut instructions = Vec::new();
    let orders_keypair;
    let mut signers = Vec::new();
    let orders_pubkey = match *orders {
        Some(pk) => pk,
        None => {
            let (orders_key, instruction) = create_dex_account(
                client,
                program_id,
                &owner.pubkey(),
                size_of::<serum_dex::state::OpenOrders>(),
            )?;
            orders_keypair = orders_key;
            signers.push(&orders_keypair);
            instructions.push(instruction);
            orders_keypair.pubkey()
        }
    };
    *orders = Some(orders_pubkey);
    instructions.push(init_open_orders_ix(
        program_id,
        &orders_pubkey,
        &owner.pubkey(),
        &state.market,
    )?);
    signers.push(owner);

    let (recent_hash, _fee_calc) = client.get_recent_blockhash()?;
    let txn = Transaction::new_signed_with_payer(
        &instructions,
        Some(&owner.pubkey()),
        &signers,
        recent_hash,
    );
    send_txn(client, &txn, false)?;
    Ok(())
}

pub fn place_order(
    client: &RpcClient,
    program_id: &Pubkey,
    payer: &Keypair,
    wallet: &Pubkey,
    state: &MarketPubkeys,
    orders: &mut Option<Pubkey>,

    new_order: NewOrderInstructionV3,
) -> Result<()> {
    let mut instructions = Vec::new();
    let orders_keypair;
    let mut signers = Vec::new();
    let orders_pubkey = match *orders {
        Some(pk) => pk,
        None => {
            let (orders_key, instruction) = create_dex_account(
                client,
                program_id,
                &payer.pubkey(),
                size_of::<serum_dex::state::OpenOrders>(),
            )?;
            orders_keypair = orders_key;
            signers.push(&orders_keypair);
            instructions.push(instruction);
            orders_keypair.pubkey()
        }
    };
    *orders = Some(orders_pubkey);
    let _side = new_order.side;
    let data = MarketInstruction::NewOrderV3(new_order).pack();
    let instruction = Instruction {
        program_id: *program_id,
        data,
        accounts: vec![
            AccountMeta::new(*state.market, false),
            AccountMeta::new(orders_pubkey, false),
            AccountMeta::new(*state.req_q, false),
            AccountMeta::new(*state.event_q, false),
            AccountMeta::new(*state.bids, false),
            AccountMeta::new(*state.asks, false),
            AccountMeta::new(*wallet, false),
            AccountMeta::new_readonly(payer.pubkey(), true),
            AccountMeta::new(*state.coin_vault, false),
            AccountMeta::new(*state.pc_vault, false),
            AccountMeta::new_readonly(spl_token::ID, false),
            AccountMeta::new_readonly(solana_sdk::sysvar::rent::ID, false),
        ],
    };
    instructions.push(instruction);
    signers.push(payer);

    let (recent_hash, _fee_calc) = client.get_recent_blockhash()?;
    let txn = Transaction::new_signed_with_payer(
        &instructions,
        Some(&payer.pubkey()),
        &signers,
        recent_hash,
    );
    send_txn(client, &txn, false)?;
    Ok(())
}

fn settle_funds(
    client: &RpcClient,
    program_id: &Pubkey,
    payer: &Keypair,
    state: &MarketPubkeys,
    signer: Option<&Keypair>,
    orders: &Pubkey,
    coin_wallet: &Pubkey,
    pc_wallet: &Pubkey,
) -> Result<()> {
    let data = MarketInstruction::SettleFunds.pack();
    let instruction = Instruction {
        program_id: *program_id,
        data,
        accounts: vec![
            AccountMeta::new(*state.market, false),
            AccountMeta::new(*orders, false),
            AccountMeta::new_readonly(signer.unwrap_or(payer).pubkey(), true),
            AccountMeta::new(*state.coin_vault, false),
            AccountMeta::new(*state.pc_vault, false),
            AccountMeta::new(*coin_wallet, false),
            AccountMeta::new(*pc_wallet, false),
            AccountMeta::new_readonly(*state.vault_signer_key, false),
            AccountMeta::new_readonly(spl_token::ID, false),
        ],
    };
    let (recent_hash, _fee_calc) = client.get_recent_blockhash()?;
    let mut signers = vec![payer];
    if let Some(s) = signer {
        signers.push(s);
    }
    let txn = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&payer.pubkey()),
        &signers,
        recent_hash,
    );
    let mut i = 0;
    loop {
        i += 1;
        assert!(i < 10);
        debug_println!("Simulating SettleFunds instruction ...");
        let result = simulate_transaction(client, &txn, true, CommitmentConfig::single())?;
        if let Some(e) = result.value.err {
            return Err(format_err!("simulate_transaction error: {:?}", e));
        }
        debug_println!("{:#?}", result.value);
        if result.value.err.is_none() {
            break;
        }
    }
    debug_println!("Settling ...");
    send_txn(client, &txn, false)?;
    Ok(())
}

pub fn list_market(
    client: &RpcClient,
    program_id: &Pubkey,
    payer: &Keypair,
    coin_mint: &Pubkey,
    pc_mint: &Pubkey,
    coin_lot_size: u64,
    pc_lot_size: u64,
) -> Result<MarketPubkeys> {
    let (listing_keys, mut instructions) =
        gen_listing_params(client, program_id, &payer.pubkey(), coin_mint, pc_mint)?;
    let ListingKeys {
        market_key,
        req_q_key,
        event_q_key,
        bids_key,
        asks_key,
        vault_signer_pk,
        vault_signer_nonce,
    } = listing_keys;

    debug_println!("Creating coin vault...");
    let coin_vault = create_token_account(client, coin_mint, &vault_signer_pk, payer)?;
    debug_println!("Created account: {} ...", coin_vault.pubkey());

    debug_println!("Creating pc vault...");
    let pc_vault = create_token_account(client, pc_mint, &listing_keys.vault_signer_pk, payer)?;
    debug_println!("Created account: {} ...", pc_vault.pubkey());

    let init_market_instruction = serum_dex::instruction::initialize_market(
        &market_key.pubkey(),
        program_id,
        coin_mint,
        pc_mint,
        &coin_vault.pubkey(),
        &pc_vault.pubkey(),
        &bids_key.pubkey(),
        &asks_key.pubkey(),
        &req_q_key.pubkey(),
        &event_q_key.pubkey(),
        coin_lot_size,
        pc_lot_size,
        vault_signer_nonce,
        100,
    )?;
    debug_println!(
        "initialize_market_instruction: {:#?}",
        &init_market_instruction
    );

    instructions.push(init_market_instruction);

    let (recent_hash, _fee_calc) = client.get_recent_blockhash()?;
    let signers = vec![
        payer,
        &market_key,
        &req_q_key,
        &event_q_key,
        &bids_key,
        &asks_key,
        &req_q_key,
        &event_q_key,
    ];
    let txn = Transaction::new_signed_with_payer(
        &instructions,
        Some(&payer.pubkey()),
        &signers,
        recent_hash,
    );

    debug_println!("txn:\n{:#x?}", txn);
    let result = simulate_transaction(client, &txn, true, CommitmentConfig::single())?;
    if let Some(e) = result.value.err {
        return Err(format_err!("simulate_transaction error: {:?}", e));
    }
    debug_println!("{:#?}", result.value);
    debug_println!("Listing {} ...", market_key.pubkey());
    send_txn(client, &txn, false)?;

    Ok(MarketPubkeys {
        market: Box::new(market_key.pubkey()),
        req_q: Box::new(req_q_key.pubkey()),
        event_q: Box::new(event_q_key.pubkey()),
        bids: Box::new(bids_key.pubkey()),
        asks: Box::new(asks_key.pubkey()),
        coin_vault: Box::new(coin_vault.pubkey()),
        pc_vault: Box::new(pc_vault.pubkey()),
        vault_signer_key: Box::new(vault_signer_pk),
    })
}

struct ListingKeys {
    market_key: Keypair,
    req_q_key: Keypair,
    event_q_key: Keypair,
    bids_key: Keypair,
    asks_key: Keypair,
    vault_signer_pk: Pubkey,
    vault_signer_nonce: u64,
}

fn gen_listing_params(
    client: &RpcClient,
    program_id: &Pubkey,
    payer: &Pubkey,
    _coin_mint: &Pubkey,
    _pc_mint: &Pubkey,
) -> Result<(ListingKeys, Vec<Instruction>)> {
    let (market_key, create_market) = create_dex_account(client, program_id, payer, 376)?;
    let (req_q_key, create_req_q) = create_dex_account(client, program_id, payer, 640)?;
    let (event_q_key, create_event_q) = create_dex_account(client, program_id, payer, 1 << 20)?;
    let (bids_key, create_bids) = create_dex_account(client, program_id, payer, 1 << 16)?;
    let (asks_key, create_asks) = create_dex_account(client, program_id, payer, 1 << 16)?;
    let (vault_signer_nonce, vault_signer_pk) = {
        let mut i = 0;
        loop {
            assert!(i < 100);
            if let Ok(pk) = gen_vault_signer_key(i, &market_key.pubkey(), program_id) {
                break (i, pk);
            }
            i += 1;
        }
    };
    let info = ListingKeys {
        market_key,
        req_q_key,
        event_q_key,
        bids_key,
        asks_key,
        vault_signer_pk,
        vault_signer_nonce,
    };
    let instructions = vec![
        create_market,
        create_req_q,
        create_event_q,
        create_bids,
        create_asks,
    ];
    Ok((info, instructions))
}

fn create_dex_account(
    client: &RpcClient,
    program_id: &Pubkey,
    payer: &Pubkey,
    unpadded_len: usize,
) -> Result<(Keypair, Instruction)> {
    let len = unpadded_len + 12;
    let key = Keypair::generate(&mut OsRng);
    let create_account_instr = solana_sdk::system_instruction::create_account(
        payer,
        &key.pubkey(),
        client.get_minimum_balance_for_rent_exemption(len)?,
        len as u64,
        program_id,
    );
    Ok((key, create_account_instr))
}

pub fn match_orders(
    client: &RpcClient,
    program_id: &Pubkey,
    payer: &Keypair,
    state: &MarketPubkeys,
    coin_wallet: &Pubkey,
    pc_wallet: &Pubkey,
) -> Result<()> {
    let instruction_data: Vec<u8> = MarketInstruction::MatchOrders(2).pack();

    let instruction = Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(*state.market, false),
            AccountMeta::new(*state.req_q, false),
            AccountMeta::new(*state.event_q, false),
            AccountMeta::new(*state.bids, false),
            AccountMeta::new(*state.asks, false),
            AccountMeta::new(*coin_wallet, false),
            AccountMeta::new(*pc_wallet, false),
        ],
        data: instruction_data,
    };

    let (recent_hash, _fee_calc) = client.get_recent_blockhash()?;
    let txn = Transaction::new_signed_with_payer(
        std::slice::from_ref(&instruction),
        Some(&payer.pubkey()),
        &[payer],
        recent_hash,
    );

    debug_println!("Simulating order matching ...");
    let result = simulate_transaction(&client, &txn, true, CommitmentConfig::single())?;
    if let Some(e) = result.value.err {
        return Err(format_err!("simulate_transaction error: {:?}", e));
    }
    debug_println!("{:#?}", result.value);
    if result.value.err.is_none() {
        debug_println!("Matching orders ...");
        send_txn(client, &txn, false)?;
    }
    Ok(())
}

fn create_account(
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

    debug_println!("Creating account: {} ...", spl_account.pubkey());
    send_txn(client, &txn, false)?;
    Ok(spl_account)
}

fn mint_to_existing_account(
    client: &RpcClient,
    payer: &Keypair,
    minting_key: &Keypair,
    mint: &Pubkey,
    recipient: &Pubkey,
    quantity: u64,
) -> Result<()> {
    let signers = vec![payer, minting_key];

    let mint_tokens_instr = token_instruction::mint_to(
        &spl_token::ID,
        mint,
        recipient,
        &minting_key.pubkey(),
        &[],
        quantity,
    )?;

    let instructions = vec![mint_tokens_instr];
    let (recent_hash, _fee_calc) = client.get_recent_blockhash()?;
    let txn = Transaction::new_signed_with_payer(
        &instructions,
        Some(&payer.pubkey()),
        &signers,
        recent_hash,
    );
    send_txn(client, &txn, false)?;
    Ok(())
}

fn initialize_token_account(client: &RpcClient, mint: &Pubkey, owner: &Keypair) -> Result<Keypair> {
    let recip_keypair = Keypair::generate(&mut OsRng);
    let lamports = client.get_minimum_balance_for_rent_exemption(spl_token::state::Account::LEN)?;
    let create_recip_instr = solana_sdk::system_instruction::create_account(
        &owner.pubkey(),
        &recip_keypair.pubkey(),
        lamports,
        spl_token::state::Account::LEN as u64,
        &spl_token::ID,
    );
    let init_recip_instr = token_instruction::initialize_account(
        &spl_token::ID,
        &recip_keypair.pubkey(),
        mint,
        &owner.pubkey(),
    )?;
    let signers = vec![owner, &recip_keypair];
    let instructions = vec![create_recip_instr, init_recip_instr];
    let (recent_hash, _fee_calc) = client.get_recent_blockhash()?;
    let txn = Transaction::new_signed_with_payer(
        &instructions,
        Some(&owner.pubkey()),
        &signers,
        recent_hash,
    );
    send_txn(client, &txn, false)?;
    Ok(recip_keypair)
}

enum MonitorEvent {
    NumEvents(usize),
    NewConn(std::net::TcpStream),
}

async fn read_queue_length_loop(
    client: RpcClient,
    program_id: Pubkey,
    market: Pubkey,
    port: u16,
) -> Result<()> {
    let client = Arc::new(client);
    let get_data = warp::path("length").map(move || {
        let client = client.clone();
        let market_keys = get_keys_for_market(&client, &program_id, &market).unwrap();
        let event_q_data = client
            .get_account_with_commitment(&market_keys.event_q, CommitmentConfig::recent())
            .unwrap()
            .value
            .expect("Failed to retrieve account")
            .data;
        let inner: Cow<[u64]> = remove_dex_account_padding(&event_q_data).unwrap();
        let (_header, seg0, seg1) = parse_event_queue(&inner).unwrap();
        let len = seg0.len() + seg1.len();
        format!("{{ \"length\": {}  }}", len)
    });

    Ok(warp::serve(get_data).run(([127, 0, 0, 1], port)).await)
}
