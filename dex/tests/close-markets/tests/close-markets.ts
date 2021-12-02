import * as anchor from '@project-serum/anchor';
import { Program, BN } from '@project-serum/anchor';
import { TOKEN_PROGRAM_ID } from "@solana/spl-token";
import { PublicKey } from "@solana/web3.js";
import { CloseMarkets } from '../target/types/close_markets';
import { Market } from "@project-serum/serum";


const DEX_PID = new anchor.web3.PublicKey(
  "9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin",
);

describe('close-markets', () => {

  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.Provider.env());

  const program = anchor.workspace.CloseMarkets as Program<CloseMarkets>;

  let eventQueueKeypair;
  let bidsKeypair;
  let asksKeypair;

  it('Initialize Market!', async () => {
    let [pruneAuth, pruneAuthBump] = await anchor.web3.PublicKey.findProgramAddress(
      [Buffer.from("prune_auth")],
      program.programId,
    );
    let [serumMint, serumMintBump] = await anchor.web3.PublicKey.findProgramAddress(
      [Buffer.from("serum_mint")],
      program.programId,
    );
    let [usdcMint, usdcMintBump] = await anchor.web3.PublicKey.findProgramAddress(
      [
        Buffer.from("usdc_mint")],
      program.programId,
    );

    let [serumMarket, serumMarketBump] =
      await anchor.web3.PublicKey.findProgramAddress(
        [Buffer.from("serum_market")],
        program.programId,
      );
    let [requestQueue, requestQueueBump] =
      await anchor.web3.PublicKey.findProgramAddress(
        [Buffer.from("request_queue")],
        program.programId,
      );
    let [coinVault, coinVaultBump] =
      await anchor.web3.PublicKey.findProgramAddress(
        [Buffer.from("coin_vault")],
        program.programId,
      );
    let [pcVault, pcVaultBump] = await anchor.web3.PublicKey.findProgramAddress(
      [Buffer.from("pc_vault")],
      program.programId,
    );

    let [vaultSigner, vaultSignerNonce] = await getVaultSignerAndNonce(
      serumMarket,
    );
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
    // Add your test here.
    // await 
    // const tx = await program.rpc.initialize({});
    // console.log("Your transaction signature", tx);
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

  it('Close Market!', async () => {

    console.log("payer account balance: ", (await program.provider.connection.getBalance(program.provider.wallet.publicKey)).toString());

    let [pruneAuth] = await anchor.web3.PublicKey.findProgramAddress(
      [Buffer.from("prune_auth")],
      program.programId,
    );
    let [serumMarket] =
      await anchor.web3.PublicKey.findProgramAddress(
        [Buffer.from("serum_market")],
        program.programId,
      );
    let [requestQueue] =
      await anchor.web3.PublicKey.findProgramAddress(
        [Buffer.from("request_queue")],
        program.programId,
      );

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
      }
    })

    console.log("payer account balance: ", (await program.provider.connection.getBalance(program.provider.wallet.publicKey)).toString());


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
