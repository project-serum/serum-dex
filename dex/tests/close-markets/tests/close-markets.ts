import * as anchor from "@project-serum/anchor";
import { Program, BN } from "@project-serum/anchor";
import {
  ASSOCIATED_TOKEN_PROGRAM_ID,
  Token,
  TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import {
  PublicKey,
  Keypair,
  SystemProgram,
  Transaction,
  Account,
} from "@solana/web3.js";
// import { CloseMarkets } from '../target/types/close_markets';
import {
  Market,
  TokenInstructions,
  OpenOrders,
  DexInstructions,
} from "@project-serum/serum";

const DEX_PID = new anchor.web3.PublicKey(
  "9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin",
);

async function sendTransaction(connection, transaction, signers) {
  const signature = await connection.sendTransaction(transaction, signers, {
    skipPreflight: true,
  });
  const { value } = await connection.confirmTransaction(signature, "processed");
  if (value === null || value === void 0 ? void 0 : value.err) {
    throw new Error(JSON.stringify(value.err));
  }
  return signature;
}

async function crankEventQueue(provider, marketClient) {
  let eq = await marketClient.loadEventQueue(provider.connection);
  let count = 0;
  console.log(eq.length);
  while (eq.length > 0) {
    console.log("begin");
    const accounts = new Set();
    for (const event of eq) {
      accounts.add(event.openOrders.toBase58());
      // TODO in Daffy's code they have a max of 10 pubkey per consumeEvents call
    }
    let orderedAccounts = Array.from(accounts)
      .map((s) => new PublicKey(s))
      .sort((a, b) => a.toBuffer().swap64().compare(b.toBuffer().swap64()));
    console.log(
      "Here are the ordered accounts to be processed",
      orderedAccounts,
    );
    let openOrdersRaw = await provider.connection.getAccountInfo(
      orderedAccounts[0],
    );
    let thisOpenOrders = OpenOrders.fromAccountInfo(
      orderedAccounts[0],
      openOrdersRaw,
      DEX_PID,
    );
    console.log("loaded up open orders: ", thisOpenOrders.owner);

    const tx = new anchor.web3.Transaction();
    tx.add(marketClient.makeConsumeEventsInstruction(orderedAccounts, 20));
    console.log("TEST");
    await provider.send(tx);
    eq = await marketClient.loadEventQueue(provider.connection);
    console.log("WEEE");
    console.log(eq.length);
    console.log("end");
    count += 1;
    if (count > 4) {
      console.log(orderedAccounts);
      break;
    }
  }
}

async function createMintToAccountInstrs(
  mint,
  destination,
  amount,
  mintAuthority,
) {
  return [
    TokenInstructions.mintTo({
      mint,
      destination,
      amount,
      mintAuthority,
    }),
  ];
}

async function createTokenAccountInstrs(
  provider,
  newAccountPubkey,
  mint,
  owner,
  lamports,
) {
  if (lamports === undefined) {
    lamports = await provider.connection.getMinimumBalanceForRentExemption(165);
  }
  return [
    SystemProgram.createAccount({
      fromPubkey: provider.wallet.publicKey,
      newAccountPubkey,
      space: 165,
      lamports,
      programId: TokenInstructions.TOKEN_PROGRAM_ID,
    }),
    TokenInstructions.initializeAccount({
      account: newAccountPubkey,
      mint,
      owner,
    }),
  ];
}

async function createTokenAccount(provider, mint, owner) {
  const vault = new Keypair();
  const tx = new Transaction();
  tx.add(
    ...(await createTokenAccountInstrs(provider, vault.publicKey, mint, owner)),
  );
  await provider.send(tx, [vault]);
  return vault.publicKey;
}

async function mintToAccount(
  provider,
  mint,
  destination,
  amount,
  mintAuthority,
) {
  // mint authority is the provider
  const tx = new Transaction();
  tx.add(
    ...(await createMintToAccountInstrs(
      mint,
      destination,
      amount,
      mintAuthority,
    )),
  );
  await provider.send(tx, []);
  return;
}

describe("close-markets", () => {
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.Provider.env());

  const program = anchor.workspace.CloseMarkets as Program<any>;

  let secondarySigner = new Account();
  let eventQueueKeypair;
  let bidsKeypair;
  let asksKeypair;
  let secondarySignerUsdcPubkey;
  let signerUsdcPubkey;
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
  let openOrdersTwo;

  // let usdcClient;
  // let openOrders, openOrdersBump, openOrdersInitAuthority, openOrdersBumpinit;

  // it("BOILERPLATE: Initializes an orderbook", async () => {
  //   const { marketProxyClient, godA, godUsdc, usdc } = await genesis({
  //     provider,
  //     proxyProgramId: program.programId,
  //   });
  //   marketProxy = marketProxyClient;
  //   usdcAccount = godUsdc;
  //   tokenAccount = godA;

  //   usdcClient = new Token(
  //     provider.connection,
  //     usdc,
  //     TOKEN_PROGRAM_ID,
  //     provider.wallet.payer,
  //   );

  //   referral = await usdcClient.createAccount(REFERRAL_AUTHORITY);
  // });

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

    let serumMarketData = await Market.load(
      program.provider.connection,
      serumMarket,
      {},
      DEX_PID,
    );
    // TODO assert Market._decoded.accountFlags.initialized = true
    // console.log("serum market data", serumMarketData);
  });

  it("Mints Tokens!", async () => {
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
  });

  it("Secondary Account Attempts To Order", async () => {
    const tokenAccount =
      await program.provider.connection.getTokenAccountBalance(
        secondarySignerUsdcPubkey,
      );

    const hello = await program.provider.connection.getAccountInfo(
      secondarySigner.publicKey,
    );

    [openOrders] = await anchor.web3.PublicKey.findProgramAddress(
      [secondarySigner.publicKey.toBuffer(), Buffer.from("open_orders")],
      program.programId,
    );

    [openOrdersTwo] = await anchor.web3.PublicKey.findProgramAddress(
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
        openOrders: openOrdersTwo,
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

    try {
      await market._sendTransaction(
        program.provider.connection,
        newTransaction,
        [secondarySigner],
      );
    } catch (e) {
      console.error(e);
    }

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

    await program.rpc.pruneOpenOrders({
      accounts: {
        payer: program.provider.wallet.publicKey,
        pruneAuth,
        serumMarket,
        requestQueue,
        eventQueue: eventQueueKeypair.publicKey,
        bids: bidsKeypair.publicKey,
        asks: asksKeypair.publicKey,
        openOrders: openOrdersTwo,
        systemProgram: anchor.web3.SystemProgram.programId,
        dexProgram: DEX_PID,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
      },
    });

    const pie = await market.loadAsks(program.provider.connection);
    const pie2 = await market.loadBids(program.provider.connection);

    await crankEventQueue(program.provider, market);
  });
  it("Close Market!", async () => {
    let market = await Market.load(
      program.provider.connection,
      serumMarket,
      undefined,
      DEX_PID,
    );
    let { baseVault, quoteVault } = market.decoded;

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

    console.log("markte is closed!!");

    const recentBlockhash = await (
      await program.provider.connection.getRecentBlockhash()
    ).blockhash;

    const newTransaction = new Transaction({
      recentBlockhash,
    });

    console.log(baseVault, quoteVault);
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

    newTransaction.add(getSettle);

    console.log(
      "payer account balance: ",
      (
        await program.provider.connection.getBalance(secondarySigner.publicKey)
      ).toString(),
    );

    await program.provider.connection.sendTransaction(newTransaction, [
      secondarySigner,
    ]);

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

    await program.provider.connection.sendTransaction(secondTxn, [
      secondarySigner,
    ]);

    console.log(
      "payer account balance: ",
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
