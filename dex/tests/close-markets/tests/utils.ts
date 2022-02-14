import * as anchor from "@project-serum/anchor";

import { PublicKey, Transaction } from "@solana/web3.js";

import { TokenInstructions, OpenOrders } from "@project-serum/serum";

const DEX_PID = new anchor.web3.PublicKey(
  "9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin",
);

export function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

export async function mintToAccount(
  provider,
  mint,
  destination,
  amount,
  mintAuthority,
) {
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

export async function crankEventQueue(provider, marketClient) {
  let eq = await marketClient.loadEventQueue(provider.connection);
  let count = 0;
  while (eq.length > 0) {
    const accounts = new Set();
    for (const event of eq) {
      accounts.add(event.openOrders.toBase58());
    }
    let orderedAccounts = Array.from(accounts)
      .map((s) => new PublicKey(s))
      .sort((a, b) => a.toBuffer().swap64().compare(b.toBuffer().swap64()));

    let openOrdersRaw = await provider.connection.getAccountInfo(
      orderedAccounts[0],
    );
    OpenOrders.fromAccountInfo(orderedAccounts[0], openOrdersRaw, DEX_PID);

    const tx = new anchor.web3.Transaction();
    tx.add(marketClient.makeConsumeEventsInstruction(orderedAccounts, 20));
    await provider.send(tx);
    eq = await marketClient.loadEventQueue(provider.connection);
    console.log(eq.length);
    count += 1;
    if (count > 4) {
      break;
    }
  }
}

export async function createMintToAccountInstrs(
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
