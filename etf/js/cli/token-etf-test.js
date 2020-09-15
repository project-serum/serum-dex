// @flow

import fs from 'mz/fs';
import semver from 'semver';
import { Account, Connection, BpfLoader, PublicKey, Token, BPF_LOADER_PROGRAM_ID } from '@solana/web3.js';

import { TokenEtf } from '../client/token-etf';
import { Store } from '../client/util/store';
import { newAccountWithLamports } from '../client/util/new-account-with-lamports';
import { url } from '../url';
import { sleep } from '../client/util/sleep';

// The following globals are created by `createTokenEtf` and used by subsequent tests
// Token etf
let tokenEtf: TokenEtf;
// authority of the token and accounts
let authority: PublicKey;
// nonce used to generate the authority public key
let nonce: number;
// owner of the user accounts
let owner: Account;
// Token pool
let tokenPool: Token;
let tokenAccountPool: PublicKey;
// Tokens etfped
let mintA: Token;
let mintB: Token;
let tokenAccountA: PublicKey;
let tokenAccountB: PublicKey;

// Initial amount in each etf token
const BASE_AMOUNT = 1000;
// Amount passed to instructions
const USER_AMOUNT = 100;

function assert(condition, message) {
  if (!condition) {
    console.log(Error().stack + ':token-test.js');
    throw message || 'Assertion failed';
  }
}

let connection;
async function getConnection(): Promise<Connection> {
  if (connection) return connection;

  let newConnection = new Connection(url, 'recent',);
  const version = await newConnection.getVersion();

  // commitment params are only supported >= 0.21.0
  const solanaCoreVersion = version['solana-core'].split(' ')[0];
  if (semver.gte(solanaCoreVersion, '0.21.0')) {
    newConnection = new Connection(url, 'recent');
  }

  // eslint-disable-next-line require-atomic-updates
  connection = newConnection;
  console.log('Connection to cluster established:', url, version);
  return newConnection;
}

async function loadProgram(connection: Connection, path: string): Promise<PublicKey> {
  const NUM_RETRIES = 500; /* allow some number of retries */
  const data = await fs.readFile(path
  );
  const { feeCalculator } = await connection.getRecentBlockhash();
  const balanceNeeded =
    feeCalculator.lamportsPerSignature *
    (BpfLoader.getMinNumSignatures(data.length) + NUM_RETRIES) +
    (await connection.getMinimumBalanceForRentExemption(data.length));

  const from = await newAccountWithLamports(connection, balanceNeeded);
  const program_account = new Account();
  console.log('Loading program:', path);
  await BpfLoader.load(connection, from, program_account, data, BPF_LOADER_PROGRAM_ID);
  return program_account.publicKey;
}

async function GetPrograms(connection: Connection): Promise<[PublicKey, PublicKey]> {
  const store = new Store();
  let tokenProgramId = null;
  let tokenEtfProgramId = null;
  try {
    const config = await store.load('config.json');
    console.log('Using pre-loaded Token and Token-etf programs');
    console.log('  Note: To reload programs remove client/util/store/config.json');
    tokenProgramId = new PublicKey(config.tokenProgramId);
    tokenEtfProgramId = new PublicKey(config.tokenEtfProgramId);
  } catch (err) {
    tokenProgramId = await loadProgram(connection, '../../target/bpfel-unknown-unknown/release/spl_token.so');
    tokenEtfProgramId = await loadProgram(connection, '../../target/bpfel-unknown-unknown/release/serum_etf.so');
    await store.save('config.json', {
      tokenProgramId: tokenProgramId.toString(),
      tokenEtfProgramId: tokenEtfProgramId.toString()
    });
  }
  return [tokenProgramId, tokenEtfProgramId];
}

export async function loadPrograms(): Promise<void> {
  const connection = await getConnection();
  const [tokenProgramId, tokenEtfProgramId] = await GetPrograms(connection);

  console.log('Token Program ID', tokenProgramId.toString());
  console.log('Token-etf Program ID', tokenEtfProgramId.toString());
}

export async function createTokenEtf(): Promise<void> {
  const connection = await getConnection();
  const [tokenProgramId, tokenEtfProgramId] = await GetPrograms(connection);
  const payer = await newAccountWithLamports(connection, 100000000000 /* wag */);
  owner = await newAccountWithLamports(connection, 100000000000 /* wag */);
  const tokenEtfAccount = new Account();

  [authority, nonce] = await PublicKey.findProgramAddress(
    [tokenEtfAccount.publicKey.toBuffer()],
    tokenEtfProgramId
  );

  console.log('creating pool mint');
  tokenPool = await Token.createMint(
    connection,
    payer,
    authority,
    null,
    2,
    tokenProgramId,
  );

  console.log('creating pool account');
  tokenAccountPool = await tokenPool.createAccount(owner.publicKey);

  console.log('creating token A');
  mintA = await Token.createMint(
    connection,
    payer,
    owner.publicKey,
    null,
    2,
    tokenProgramId,
  );

  console.log('creating token A account');
  tokenAccountA = await mintA.createAccount(authority);
  console.log('minting token A to etf');
  await mintA.mintTo(tokenAccountA, owner, [], BASE_AMOUNT);

  console.log('creating token B');
  mintB = await Token.createMint(
    connection,
    payer,
    owner.publicKey,
    null,
    2,
    tokenProgramId,
  );

  console.log('creating token B account');
  tokenAccountB = await mintB.createAccount(authority);
  console.log('minting token B to etf');
  await mintB.mintTo(tokenAccountB, owner, [], BASE_AMOUNT);

  console.log('creating token etf');
  const etfPayer = await newAccountWithLamports(connection, 100000000000 /* wag */);
  tokenEtf = await TokenEtf.createTokenEtf(
    connection,
    etfPayer,
    tokenEtfAccount,
    authority,
    tokenAccountA,
    tokenAccountB,
    tokenPool.publicKey,
    tokenAccountPool,
    tokenProgramId,
    nonce,
    1,
    4,
    tokenEtfProgramId
  );

  console.log('getting token etf');
  const etfInfo = await tokenEtf.getInfo();
  assert(etfInfo.tokenAccountA.equals(tokenAccountA));
  assert(etfInfo.tokenAccountB.equals(tokenAccountB));
  assert(etfInfo.tokenPool.equals(tokenPool.publicKey));
}

export async function deposit(): Promise<void> {
  console.log('Creating depositor token a account');
  let userAccountA = await mintA.createAccount(owner.publicKey);
  await mintA.mintTo(userAccountA, owner, [], USER_AMOUNT);
  await mintA.approve(
    userAccountA,
    authority,
    owner,
    [],
    USER_AMOUNT,
  );
  console.log('Creating depositor token b account');
  let userAccountB = await mintB.createAccount(owner.publicKey);
  await mintB.mintTo(userAccountB, owner, [], USER_AMOUNT);
  await mintB.approve(
    userAccountB,
    authority,
    owner,
    [],
    USER_AMOUNT,
  );
  console.log('Creating depositor pool token account');
  let newAccountPool = await tokenPool.createAccount(owner.publicKey);
  const [tokenProgramId,] = await GetPrograms(connection);

  console.log('Depositing into etf');
  await tokenEtf.deposit(
    authority,
    userAccountA,
    userAccountB,
    tokenAccountA,
    tokenAccountB,
    tokenPool.publicKey,
    newAccountPool,
    tokenProgramId,
    USER_AMOUNT,
  );

  let info;
  info = await mintA.getAccountInfo(userAccountA);
  assert(info.amount.toNumber() == 0);
  info = await mintB.getAccountInfo(userAccountB);
  assert(info.amount.toNumber() == 0);
  info = await mintA.getAccountInfo(tokenAccountA);
  assert(info.amount.toNumber() == BASE_AMOUNT + USER_AMOUNT);
  info = await mintB.getAccountInfo(tokenAccountB);
  assert(info.amount.toNumber() == BASE_AMOUNT + USER_AMOUNT);
  info = await tokenPool.getAccountInfo(newAccountPool);
  assert(info.amount.toNumber() == USER_AMOUNT);
}

export async function withdraw(): Promise<void> {
  console.log('Creating withdraw token A account');
  let userAccountA = await mintA.createAccount(owner.publicKey);
  console.log('Creating withdraw token B account');
  let userAccountB = await mintB.createAccount(owner.publicKey);

  console.log('Approving withdrawal from pool account');
  await tokenPool.approve(
    tokenAccountPool,
    authority,
    owner,
    [],
    USER_AMOUNT,
  );
  const [tokenProgramId,] = await GetPrograms(connection);

  console.log('Withdrawing pool tokens for A and B tokens');
  await tokenEtf.withdraw(
    authority,
    tokenPool.publicKey,
    tokenAccountPool,
    tokenAccountA,
    tokenAccountB,
    userAccountA,
    userAccountB,
    tokenProgramId,
    USER_AMOUNT
  );

  let info = await tokenPool.getAccountInfo(tokenAccountPool);
  assert(info.amount.toNumber() == BASE_AMOUNT - USER_AMOUNT);
  info = await mintA.getAccountInfo(tokenAccountA);
  assert(info.amount.toNumber() == BASE_AMOUNT);
  info = await mintB.getAccountInfo(tokenAccountB);
  assert(info.amount.toNumber() == BASE_AMOUNT);
  info = await mintA.getAccountInfo(userAccountA);
  assert(info.amount.toNumber() == USER_AMOUNT);
  info = await mintB.getAccountInfo(userAccountB);
  assert(info.amount.toNumber() == USER_AMOUNT);
}
