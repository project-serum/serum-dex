/**
 * @flow
 */

import assert from 'assert';
import BN from 'bn.js';
import * as BufferLayout from 'buffer-layout';
import {
  Account,
  PublicKey,
  SystemProgram,
  Transaction,
  TransactionInstruction,
} from '@solana/web3.js';
import type {Connection, TransactionSignature} from '@solana/web3.js';

import * as Layout from './layout';
import {sendAndConfirmTransaction} from './util/send-and-confirm-transaction';

/**
 * Some amount of tokens
 */
export class Numberu64 extends BN {
  /**
   * Convert to Buffer representation
   */
  toBuffer(): Buffer {
    const a = super.toArray().reverse();
    const b = Buffer.from(a);
    if (b.length === 8) {
      return b;
    }
    assert(b.length < 8, 'Numberu64 too large');

    const zeroPad = Buffer.alloc(8);
    b.copy(zeroPad);
    return zeroPad;
  }

  /**
   * Construct a Numberu64 from Buffer representation
   */
  static fromBuffer(buffer: Buffer): Numberu64 {
    assert(buffer.length === 8, `Invalid buffer length: ${buffer.length}`);
    return new BN(
      [...buffer]
        .reverse()
        .map(i => `00${i.toString(16)}`.slice(-2))
        .join(''),
      16,
    );
  }
}

/**
 * Information about a token etf
 */
type TokenEtfInfo = {|
  /**
   * Nonce. Used to generate the valid program address in the program
   */
  nonce: number,

  /**
   * Token A. The Liquidity token is issued against this value.
   */
  tokenAccountA: PublicKey,

  /**
   * Token B
   */
  tokenAccountB: PublicKey,

  /**
   * Pool tokens are issued when A or B tokens are deposited
   * Pool tokens can be withdrawn back to the original A or B token
   */
  tokenPool: PublicKey,
|};

/**
 * @private
 */
const TokenEtfLayout = BufferLayout.struct([
  BufferLayout.u8('isInitialized'),
  BufferLayout.u8('nonce'),
  Layout.publicKey('tokenAccountA'),
  Layout.publicKey('tokenAccountB'),
  Layout.publicKey('tokenPool'),
]);

/**
 * An ERC20-like Token
 */
export class TokenEtf {
  /**
   * @private
   */
  connection: Connection;

  /**
   * The public key identifying this token
   */
  tokenEtf: PublicKey;

  /**
   * Program Identifier for the Token Etf program
   */
  programId: PublicKey;

  /**
   * Fee payer
   */
  payer: Account;

  /**
   * Create a Token object attached to the specific token
   *
   * @param connection The connection to use
   * @param token Public key of the token
   * @param programId Optional token programId, uses the system programId by default
   * @param payer Payer of fees
   */
  constructor(connection: Connection, tokenEtf: PublicKey, programId: PublicKey, payer: Account) {
    Object.assign(this, {connection, tokenEtf, programId, payer});
  }

  /**
   * Get the minimum balance for the token etf account to be rent exempt
   *
   * @return Number of lamports required
   */
  static async getMinBalanceRentForExemptTokenEtf(
    connection: Connection,
  ): Promise<number> {
    return await connection.getMinimumBalanceForRentExemption(
      TokenEtfLayout.span,
    );
  }

  /**
   * Create a new Token Etf
   *
   * @param connection The connection to use
   * @param payer Pays for the transaction
   * @param tokenEtfAccount The token etf account
   * @param authority The authority over the etf and accounts
   * @param tokenAccountA: The Etf's Token A account
   * @param tokenAccountB: The Etf's Token B account
   * @param tokenPool The pool token
   * @param tokenAccountPool The pool token account
   * @param tokenProgramId The program id of the token program
   * @param programId Program ID of the token-etf program
   * @return Token object for the newly minted token, Public key of the account holding the total supply of new tokens
   */
  static async createTokenEtf(
    connection: Connection,
    payer: Account,
    tokenEtfAccount: Account,
    authority: PublicKey,
    tokenAccountA: PublicKey,
    tokenAccountB: PublicKey,
    tokenPool: PublicKey,
    tokenAccountPool: PublicKey,
    tokenProgramId: PublicKey,
    nonce: number,
    programId: PublicKey,
  ): Promise<TokenEtf> {
    let transaction;
    const tokenEtf = new TokenEtf(connection, tokenEtfAccount.publicKey, programId, payer);

    // Allocate memory for the account
    const balanceNeeded = await TokenEtf.getMinBalanceRentForExemptTokenEtf(
      connection,
    );
    transaction = new Transaction();
    transaction.add(SystemProgram.createAccount({
      fromPubkey: payer.publicKey,
      newAccountPubkey: tokenEtfAccount.publicKey,
      lamports: balanceNeeded,
      space: TokenEtfLayout.span,
      programId,
    }));
    await sendAndConfirmTransaction(
      'createAccount',
      connection,
      transaction,
      payer,
      tokenEtfAccount,
    );

    let keys = [
      {pubkey: tokenEtfAccount.publicKey, isSigner: true, isWritable: true},
      {pubkey: authority, isSigner: false, isWritable: false},
      {pubkey: tokenAccountA, isSigner: false, isWritable: false},
      {pubkey: tokenAccountB, isSigner: false, isWritable: false},
      {pubkey: tokenPool, isSigner: false, isWritable: true},
      {pubkey: tokenAccountPool, isSigner: false, isWritable: true},
      {pubkey: tokenProgramId, isSigner: false, isWritable: false},
    ];
    const commandDataLayout = BufferLayout.struct([
      BufferLayout.u8('instruction'),
      BufferLayout.nu64('feeNumerator'),
      BufferLayout.nu64('feeDenominator'),
      BufferLayout.u8('nonce'),
    ]);
    let data = Buffer.alloc(1024);
    {
      const encodeLength = commandDataLayout.encode(
        {
          instruction: 0, // InitializeEtf instruction
          feeNumerator,
          feeDenominator,
          nonce,
        },
        data,
      );
      data = data.slice(0, encodeLength);
    }
    transaction = new Transaction().add({
      keys,
      programId,
      data,
    });
    await sendAndConfirmTransaction(
      'InitializeEtf',
      connection,
      transaction,
      payer,
      tokenEtfAccount
    );

    return tokenEtf;
  }

  /**
   * Retrieve tokenEtf information
   */
  async getInfo(): Promise<TokenEtfInfo> {
    const accountInfo = await this.connection.getAccountInfo(this.tokenEtf);
    if (accountInfo === null) {
      throw new Error('Failed to find token etf account');
    }
    if (!accountInfo.owner.equals(this.programId)) {
      throw new Error(
        `Invalid token etf owner: ${JSON.stringify(accountInfo.owner)}`,
      );
    }

    const data = Buffer.from(accountInfo.data);
    const tokenEtfInfo = TokenEtfLayout.decode(data);
    if (!tokenEtfInfo.isInitialized) {
      throw new Error(`Invalid token etf state`);
    }
    // already properly filled in
    // tokenEtfInfo.nonce = tokenEtfInfo.nonce;
    tokenEtfInfo.tokenAccountA = new PublicKey(tokenEtfInfo.tokenAccountA);
    tokenEtfInfo.tokenAccountB = new PublicKey(tokenEtfInfo.tokenAccountB);
    tokenEtfInfo.tokenPool = new PublicKey(tokenEtfInfo.tokenPool);
    tokenEtfInfo.feesNumerator = Numberu64.fromBuffer(tokenEtfInfo.feesNumerator);
    tokenEtfInfo.feesDenominator = Numberu64.fromBuffer(tokenEtfInfo.feesDenominator);
    tokenEtfInfo.feeRatio = tokenEtfInfo.feesNumerator.toNumber() / tokenEtfInfo.feesDenominator.toNumber();

    return tokenEtfInfo;
  }

  /**
   * Deposit some tokens into the pool
   *
   * @param authority Authority
   * @param sourceA Source account A
   * @param sourceB Source account B
   * @param intoA Base account A to deposit into
   * @param intoB Base account B to deposit into
   * @param poolToken Pool token
   * @param poolAccount Pool account to deposit the generated tokens
   * @param tokenProgramId Token program id
   * @param amount Amount of token A to transfer, token B amount is set by the exchange rate
   */
  async deposit(
    authority: PublicKey,
    sourceA: PublicKey,
    sourceB: PublicKey,
    intoA: PublicKey,
    intoB: PublicKey,
    poolToken: PublicKey,
    poolAccount: PublicKey,
    tokenProgramId: PublicKey,
    amount: number | Numberu64,
  ): Promise<TransactionSignature> {
    return await sendAndConfirmTransaction(
      'deposit',
      this.connection,
      new Transaction().add(
        this.depositInstruction(
          authority,
          sourceA,
          sourceB,
          intoA,
          intoB,
          poolToken,
          poolAccount,
          tokenProgramId,
          amount,
        ),
      ),
      this.payer,
    );
  }

  depositInstruction(
    authority: PublicKey,
    sourceA: PublicKey,
    sourceB: PublicKey,
    intoA: PublicKey,
    intoB: PublicKey,
    poolToken: PublicKey,
    poolAccount: PublicKey,
    tokenProgramId: PublicKey,
    amount: number | Numberu64,
  ): TransactionInstruction {
    const dataLayout = BufferLayout.struct([
      BufferLayout.u8('instruction'),
      Layout.uint64('amount'),
    ]);

    const data = Buffer.alloc(dataLayout.span);
    dataLayout.encode(
      {
        instruction: 2, // Deposit instruction
        amount: new Numberu64(amount).toBuffer(),
      },
      data,
    );

    const keys = [
      {pubkey: this.tokenEtf, isSigner: false, isWritable: false},
      {pubkey: authority, isSigner: false, isWritable: false},
      {pubkey: sourceA, isSigner: false, isWritable: true},
      {pubkey: sourceB, isSigner: false, isWritable: true},
      {pubkey: intoA, isSigner: false, isWritable: true},
      {pubkey: intoB, isSigner: false, isWritable: true},
      {pubkey: poolToken, isSigner: false, isWritable: true},
      {pubkey: poolAccount, isSigner: false, isWritable: true},
      {pubkey: tokenProgramId, isSigner: false, isWritable: false},
    ];
    return new TransactionInstruction({
      keys,
      programId: this.programId,
      data,
    });
  }

  /**
   * Withdraw the token from the pool at the current ratio
   *
   * @param authority Authority
   * @param sourcePoolAccount Source pool account
   * @param poolToken Pool token
   * @param fromA Base account A to withdraw from
   * @param fromB Base account B to withdraw from
   * @param userAccountA Token A user account
   * @param userAccountB token B user account
   * @param tokenProgramId Token program id
   * @param amount Amount of token A to transfer, token B amount is set by the exchange rate
   */
  async withdraw(
    authority: PublicKey,
    poolMint: PublicKey,
    sourcePoolAccount: PublicKey,
    fromA: PublicKey,
    fromB: PublicKey,
    userAccountA: PublicKey,
    userAccountB: PublicKey,
    tokenProgramId: PublicKey,
    amount: number | Numberu64,
  ): Promise<TransactionSignature> {
    return await sendAndConfirmTransaction(
      'withdraw',
      this.connection,
      new Transaction().add(
        this.withdrawInstruction(
          authority,
          poolMint,
          sourcePoolAccount,
          fromA,
          fromB,
          userAccountA,
          userAccountB,
          tokenProgramId,
          amount,
        ),
      ),
      this.payer,
    );
  }

  withdrawInstruction(
    authority: PublicKey,
    poolMint: PublicKey,
    sourcePoolAccount: PublicKey,
    fromA: PublicKey,
    fromB: PublicKey,
    userAccountA: PublicKey,
    userAccountB: PublicKey,
    tokenProgramId: PublicKey,
    amount: number | Numberu64,
  ): TransactionInstruction {
    const dataLayout = BufferLayout.struct([
      BufferLayout.u8('instruction'),
      Layout.uint64('amount'),
    ]);

    const data = Buffer.alloc(dataLayout.span);
    dataLayout.encode(
      {
        instruction: 3, // Withdraw instruction
        amount: new Numberu64(amount).toBuffer(),
      },
      data,
    );

    const keys = [
      {pubkey: this.tokenEtf, isSigner: false, isWritable: false},
      {pubkey: authority, isSigner: false, isWritable: false},
      {pubkey: poolMint, isSigner: false, isWritable: true},
      {pubkey: sourcePoolAccount, isSigner: false, isWritable: true},
      {pubkey: fromA, isSigner: false, isWritable: true},
      {pubkey: fromB, isSigner: false, isWritable: true},
      {pubkey: userAccountA, isSigner: false, isWritable: true},
      {pubkey: userAccountB, isSigner: false, isWritable: true},
      {pubkey: tokenProgramId, isSigner: false, isWritable: false},

    ];
    return new TransactionInstruction({
      keys,
      programId: this.programId,
      data,
    });
  }
}
