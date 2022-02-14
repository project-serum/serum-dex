import * as anchor from "@project-serum/anchor";
import { Program, BN } from "@project-serum/anchor";
import {
  ASSOCIATED_TOKEN_PROGRAM_ID,
  Token,
  TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import { PublicKey, Keypair, Transaction } from "@solana/web3.js";

import { Market, OpenOrders, DexInstructions } from "@project-serum/serum";
import { crankEventQueue, mintToAccount, sleep } from "./utils";

const DEX_PID = new anchor.web3.PublicKey(
  "9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin",
);

describe("close-markets", () => {
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.Provider.env());

  const program = anchor.workspace.CloseMarkets as Program<any>;

  let secondarySigner = new Keypair();
  let eventQueueKeypair;
  let bidsKeypair;
  let asksKeypair;
  let secondarySignerUsdcPubkey;
  let signerUsdcPubkey;
  let signerSerumPubkey;

  let secondarySignerSerumPubkey;

  let pruneAuth;
  let serumMint;
  let usdcMint;
  let serumMarket;

  let serumMarketBump;
  let pruneAuthBump;
  let usdcMintBump;
  let serumMintBump;

  let requestQueue;
  let requestQueueBump;
  let coinVault;
  let coinVaultBump;

  let pcVault;
  let pcVaultBump;

  let vaultSigner;
  let vaultSignerNonce;

  let openOrders;
  let openOrdersProvider;

  it("Initialize Market!", async () => {
    [pruneAuth, pruneAuthBump] = await anchor.web3.PublicKey.findProgramAddress(
      [Buffer.from("prune_auth")],
      program.programId,
    );

    [serumMint, serumMintBump] = await anchor.web3.PublicKey.findProgramAddress(
      [Buffer.from("serum_mint")],
      program.programId,
    );
    [usdcMint, usdcMintBump] = await anchor.web3.PublicKey.findProgramAddress(
      [Buffer.from("usdc_mint")],
      program.programId,
    );

    [serumMarket, serumMarketBump] =
      await anchor.web3.PublicKey.findProgramAddress(
        [Buffer.from("serum_market")],
        program.programId,
      );
    [requestQueue, requestQueueBump] =
      await anchor.web3.PublicKey.findProgramAddress(
        [Buffer.from("request_queue")],
        program.programId,
      );
    [coinVault, coinVaultBump] = await anchor.web3.PublicKey.findProgramAddress(
      [Buffer.from("coin_vault")],
      program.programId,
    );
    [pcVault, pcVaultBump] = await anchor.web3.PublicKey.findProgramAddress(
      [Buffer.from("pc_vault")],
      program.programId,
    );

    [vaultSigner, vaultSignerNonce] = await getVaultSignerAndNonce(serumMarket);
    eventQueueKeypair = anchor.web3.Keypair.generate();
    bidsKeypair = anchor.web3.Keypair.generate();
    asksKeypair = anchor.web3.Keypair.generate();

    let bumps = new Bumps();
    bumps.pruneAuth = pruneAuthBump;
    bumps.usdcMint = usdcMintBump;
    bumps.serumMint = serumMintBump;
    bumps.serumMarket = serumMarketBump;
    bumps.requestQueue = requestQueueBump;
    bumps.coinVault = coinVaultBump;
    bumps.pcVault = pcVaultBump;
    bumps.vaultSigner = vaultSignerNonce;

    await program.rpc.initializeMarket(bumps, {
      accounts: {
        payer: program.provider.wallet.publicKey,
        pruneAuth,
        usdcMint,
        serumMint,
        serumMarket,
        requestQueue,
        coinVault,
        pcVault,
        vaultSigner,
        eventQueue: eventQueueKeypair.publicKey,
        bids: bidsKeypair.publicKey,
        asks: asksKeypair.publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
        dexProgram: DEX_PID,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
      },
      instructions: [
        anchor.web3.SystemProgram.createAccount({
          fromPubkey: program.provider.wallet.publicKey,
          newAccountPubkey: eventQueueKeypair.publicKey,
          lamports:
            await program.provider.connection.getMinimumBalanceForRentExemption(
              262144 + 12,
            ),
          space: 262144 + 12,
          programId: DEX_PID,
        }),
        anchor.web3.SystemProgram.createAccount({
          fromPubkey: program.provider.wallet.publicKey,
          newAccountPubkey: bidsKeypair.publicKey,
          lamports:
            await program.provider.connection.getMinimumBalanceForRentExemption(
              65536 + 12,
            ),
          space: 65536 + 12,
          programId: DEX_PID,
        }),
        anchor.web3.SystemProgram.createAccount({
          fromPubkey: program.provider.wallet.publicKey,
          newAccountPubkey: asksKeypair.publicKey,
          lamports:
            await program.provider.connection.getMinimumBalanceForRentExemption(
              65536 + 12,
            ),
          space: 65536 + 12,
          programId: DEX_PID,
        }),
      ],
      signers: [eventQueueKeypair, bidsKeypair, asksKeypair],
    });
  });

  it("Mints Tokens for all the accounts", async () => {
    await program.provider.connection.requestAirdrop(
      secondarySigner.publicKey,
      1_000_000_000,
    );

    secondarySignerUsdcPubkey = await Token.getAssociatedTokenAddress(
      ASSOCIATED_TOKEN_PROGRAM_ID,
      TOKEN_PROGRAM_ID,
      usdcMint,
      secondarySigner.publicKey,
    );

    let createUserUsdcInstr = Token.createAssociatedTokenAccountInstruction(
      ASSOCIATED_TOKEN_PROGRAM_ID,
      TOKEN_PROGRAM_ID,
      usdcMint,
      secondarySignerUsdcPubkey,
      secondarySigner.publicKey,

      program.provider.wallet.publicKey,
    );
    let createUserUsdcTrns = new anchor.web3.Transaction().add(
      createUserUsdcInstr,
    );
    await program.provider.send(createUserUsdcTrns);

    await mintToAccount(
      program.provider,
      usdcMint,
      secondarySignerUsdcPubkey,
      new anchor.BN(500000000),
      program.provider.wallet.publicKey,
    );

    signerUsdcPubkey = await Token.getAssociatedTokenAddress(
      ASSOCIATED_TOKEN_PROGRAM_ID,
      TOKEN_PROGRAM_ID,
      usdcMint,
      program.provider.wallet.publicKey,
    );

    let createSignerUsdcInstr = Token.createAssociatedTokenAccountInstruction(
      ASSOCIATED_TOKEN_PROGRAM_ID,
      TOKEN_PROGRAM_ID,
      usdcMint,
      signerUsdcPubkey,
      program.provider.wallet.publicKey,

      program.provider.wallet.publicKey,
    );
    let createSignerUsdcTrns = new anchor.web3.Transaction().add(
      createSignerUsdcInstr,
    );
    await program.provider.send(createSignerUsdcTrns);

    await mintToAccount(
      program.provider,
      usdcMint,
      signerUsdcPubkey,
      new anchor.BN(500000000),
      program.provider.wallet.publicKey,
    );

    secondarySignerSerumPubkey = await Token.getAssociatedTokenAddress(
      ASSOCIATED_TOKEN_PROGRAM_ID,
      TOKEN_PROGRAM_ID,
      serumMint,
      secondarySigner.publicKey,
    );

    let createUserSerumInstr = Token.createAssociatedTokenAccountInstruction(
      ASSOCIATED_TOKEN_PROGRAM_ID,
      TOKEN_PROGRAM_ID,
      serumMint,
      secondarySignerSerumPubkey,
      secondarySigner.publicKey,

      program.provider.wallet.publicKey,
    );
    let createUserSerumTrns = new anchor.web3.Transaction().add(
      createUserSerumInstr,
    );
    await program.provider.send(createUserSerumTrns);

    await mintToAccount(
      program.provider,
      serumMint,
      secondarySignerSerumPubkey,
      new anchor.BN(500000000),
      program.provider.wallet.publicKey,
    );

    signerSerumPubkey = await Token.getAssociatedTokenAddress(
      ASSOCIATED_TOKEN_PROGRAM_ID,
      TOKEN_PROGRAM_ID,
      serumMint,
      program.provider.wallet.publicKey,
    );

    let createSignerSerumInstr = Token.createAssociatedTokenAccountInstruction(
      ASSOCIATED_TOKEN_PROGRAM_ID,
      TOKEN_PROGRAM_ID,
      serumMint,
      signerSerumPubkey,
      program.provider.wallet.publicKey,
      program.provider.wallet.publicKey,
    );

    let createSignerSerum = new anchor.web3.Transaction().add(
      createSignerSerumInstr,
    );

    await program.provider.send(createSignerSerum);

    await mintToAccount(
      program.provider,
      serumMint,
      signerSerumPubkey,
      new anchor.BN(500000000),
      program.provider.wallet.publicKey,
    );
  });

  it("Fails to close a market with an outstanding order", async () => {
    [openOrders] = await anchor.web3.PublicKey.findProgramAddress(
      [secondarySigner.publicKey.toBuffer(), Buffer.from("open_orders")],
      program.programId,
    );

    [openOrdersProvider] = await anchor.web3.PublicKey.findProgramAddress(
      [
        program.provider.wallet.publicKey.toBuffer(),
        Buffer.from("open_orders"),
      ],
      program.programId,
    );

    await program.rpc.initOpenOrders({
      accounts: {
        payer: secondarySigner.publicKey,
        pruneAuth,
        serumMarket,
        openOrders,
        systemProgram: anchor.web3.SystemProgram.programId,
        dexProgram: DEX_PID,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
      },
      signers: [secondarySigner],
    });

    await program.rpc.initOpenOrders({
      accounts: {
        payer: program.provider.wallet.publicKey,
        pruneAuth,
        serumMarket,
        openOrders: openOrdersProvider,
        systemProgram: anchor.web3.SystemProgram.programId,
        dexProgram: DEX_PID,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
      },
    });

    let market = await Market.load(
      program.provider.connection,
      serumMarket,
      undefined,
      DEX_PID,
    );

    const newTransaction = new Transaction();

    await program.provider.connection.getTokenAccountBalance(
      secondarySignerUsdcPubkey,
    );

    const getInstr = await market.makePlaceOrderInstruction(
      program.provider.connection,
      {
        owner: secondarySigner,
        payer: secondarySignerUsdcPubkey,
        side: "buy",
        price: 50,
        size: 2,
        orderType: "limit",
        clientId: new BN(0),
        openOrdersAddressKey: openOrders,
      },
    );

    newTransaction.add(getInstr);

    await market._sendTransaction(program.provider.connection, newTransaction, [
      secondarySigner,
    ]);

    try {
      await program.rpc.closeMarket({
        accounts: {
          payer: program.provider.wallet.publicKey,
          pruneAuth,
          serumMarket,
          requestQueue,
          eventQueue: eventQueueKeypair.publicKey,
          bids: bidsKeypair.publicKey,
          asks: asksKeypair.publicKey,
          dexProgram: DEX_PID,
        },
      });
    } catch (e) {
      console.log("close market failed!");
    }
  });

  it("Matches an order and still fails", async () => {
    let market = await Market.load(
      program.provider.connection,
      serumMarket,
      undefined,
      DEX_PID,
    );

    const newTransaction = new Transaction();

    const getInstr = await market.makePlaceOrderInstruction(
      program.provider.connection,
      {
        owner: secondarySigner,
        payer: secondarySignerSerumPubkey,
        side: "sell",
        price: 50,
        size: 10,
        orderType: "limit",
        clientId: new BN(0),
        openOrdersAddressKey: openOrders,
      },
    );

    newTransaction.add(getInstr);

    await market._sendTransaction(program.provider.connection, newTransaction, [
      secondarySigner,
    ]);

    await crankEventQueue(program.provider, market);
    await sleep(1000);

    try {
      await program.rpc.closeMarket({
        accounts: {
          payer: program.provider.wallet.publicKey,
          pruneAuth,
          serumMarket,
          requestQueue,
          eventQueue: eventQueueKeypair.publicKey,
          bids: bidsKeypair.publicKey,
          asks: asksKeypair.publicKey,
          dexProgram: DEX_PID,
        },
      });
    } catch (e) {
      console.log("close market failed!");
    }
  });

  it("Closes a Market with no outstanding orders, no uncranked events, but an open orders account that hasn't been settled and then settles it after the market has been closed", async () => {
    let market = await Market.load(
      program.provider.connection,
      serumMarket,
      undefined,
      DEX_PID,
    );

    await program.rpc.pruneOpenOrders({
      accounts: {
        payer: secondarySigner.publicKey,
        pruneAuth,
        serumMarket,
        requestQueue,
        eventQueue: eventQueueKeypair.publicKey,
        bids: bidsKeypair.publicKey,
        asks: asksKeypair.publicKey,
        openOrders: openOrders,
        systemProgram: anchor.web3.SystemProgram.programId,
        dexProgram: DEX_PID,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
      },
    });

    await crankEventQueue(program.provider, market);

    await program.rpc.closeMarket({
      accounts: {
        payer: program.provider.wallet.publicKey,
        pruneAuth,
        serumMarket,
        requestQueue,
        eventQueue: eventQueueKeypair.publicKey,
        bids: bidsKeypair.publicKey,
        asks: asksKeypair.publicKey,
        dexProgram: DEX_PID,
      },
    });
  });

  it("Allows retrieval and settling of rent after the market closes", async () => {
    let market = await Market.load(
      program.provider.connection,
      serumMarket,
      undefined,
      DEX_PID,
    );

    let { baseVault, quoteVault } = market.decoded;

    await OpenOrders.load(program.provider.connection, openOrders, DEX_PID);

    const recentBlockhash = await (
      await program.provider.connection.getRecentBlockhash()
    ).blockhash;

    const settleTransaction = new Transaction({
      recentBlockhash,
    });

    const getSettle = DexInstructions.settleFunds({
      market: serumMarket,
      openOrders,
      owner: secondarySigner.publicKey,
      baseVault,
      quoteVault,
      vaultSigner,
      quoteWallet: secondarySignerUsdcPubkey,
      baseWallet: secondarySignerSerumPubkey,
      programId: DEX_PID,
    });

    settleTransaction.add(getSettle);

    let tokenBalanceBeforeSettling =
      await program.provider.connection.getTokenAccountBalance(
        secondarySignerUsdcPubkey,
      );

    console.log(
      "tokenBalanceBeforeSettling",
      tokenBalanceBeforeSettling.value.uiAmount,
    );

    const txnsig = await program.provider.connection.sendTransaction(
      settleTransaction,
      [secondarySigner],
    );

    await sleep(5000);

    let tokenBalanceAfterSettling =
      await program.provider.connection.getTokenAccountBalance(
        secondarySignerUsdcPubkey,
      );

    console.log(
      "tokenBalanceAfterSettling",
      tokenBalanceAfterSettling.value.uiAmount,
    );

    console.log(txnsig);

    const nextBlockhash =
      await program.provider.connection.getRecentBlockhash();

    const secondTxn = new Transaction({
      recentBlockhash: nextBlockhash.blockhash,
    });

    const getInstr = DexInstructions.closeOpenOrders({
      market: serumMarket,
      openOrders,
      solWallet: secondarySigner.publicKey,
      owner: secondarySigner.publicKey,
      programId: DEX_PID,
    });

    secondTxn.add(getInstr);

    console.log(
      "payer account balance after open orders close: ",
      (
        await program.provider.connection.getBalance(secondarySigner.publicKey)
      ).toString(),
    );
    await program.provider.connection.sendTransaction(secondTxn, [
      secondarySigner,
    ]);

    await sleep(5000);

    console.log(
      "payer account balance after open orders close: ",
      (
        await program.provider.connection.getBalance(secondarySigner.publicKey)
      ).toString(),
    );
  });

  function Bumps() {
    this.pruneAuth;
    this.usdcMint;
    this.serumMint;
    this.serumMarket;
    this.requestQueue;
    this.coinVault;
    this.pcVault;
    this.vaultSigner;
  }

  async function getVaultSignerAndNonce(market: PublicKey) {
    const nonce = new BN(0);
    while (true) {
      try {
        const vaultSigner = await anchor.web3.PublicKey.createProgramAddress(
          [market.toBuffer(), nonce.toArrayLike(Buffer, "le", 8)],
          DEX_PID,
        );
        return [vaultSigner, nonce];
      } catch (e) {
        nonce.iaddn(1);
      }
    }
  }
});
