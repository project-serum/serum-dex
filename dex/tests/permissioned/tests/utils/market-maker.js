const { Account, Transaction } = require("@project-serum/anchor").web3;
const { OpenOrdersPda } = require("@project-serum/serum");

// Dummy keypair.
const KEYPAIR = new Account([
  54,
  213,
  91,
  255,
  163,
  120,
  88,
  183,
  223,
  23,
  220,
  204,
  82,
  117,
  212,
  214,
  118,
  184,
  2,
  29,
  89,
  149,
  22,
  233,
  108,
  177,
  60,
  249,
  218,
  166,
  30,
  221,
  59,
  168,
  233,
  123,
  204,
  37,
  123,
  124,
  86,
  176,
  214,
  12,
  63,
  195,
  231,
  15,
  1,
  143,
  7,
  7,
  232,
  38,
  69,
  214,
  45,
  58,
  115,
  55,
  129,
  25,
  228,
  30,
]);

async function initOpenOrders(provider, marketProxy, marketMakerAccounts) {
  const tx = new Transaction();
  tx.add(
    marketProxy.instruction.initOpenOrders(
      marketMakerAccounts.account.publicKey,
      marketProxy.market.address,
      marketProxy.market.address, // Dummy. Replaced by middleware.
      marketProxy.market.address // Dummy. Replaced by middleware.
    )
  );
  let signers = [marketMakerAccounts.account];
  await provider.send(tx, signers);
}

async function postOrders(provider, marketProxy, marketMakerAccounts) {
  const asks = [
    [6.041, 7.8],
    [6.051, 72.3],
    [6.055, 5.4],
    [6.067, 15.7],
    [6.077, 390.0],
    [6.09, 24.0],
    [6.11, 36.3],
    [6.133, 300.0],
    [6.167, 687.8],
  ];
  const bids = [
    [6.004, 8.5],
    [5.995, 12.9],
    [5.987, 6.2],
    [5.978, 15.3],
    [5.965, 82.8],
    [5.961, 25.4],
  ];
  const openOrdersAddressKey = await OpenOrdersPda.openOrdersAddress(
    marketProxy.market.address,
    marketMakerAccounts.account.publicKey,
    marketProxy.dexProgramId,
    marketProxy.proxyProgramId
  );
  // Use an explicit signer because the provider wallet, which pays for
  // the tx, is different from the market maker wallet.
  let signers = [marketMakerAccounts.account];
  for (let k = 0; k < asks.length; k += 1) {
    let ask = asks[k];
    const tx = new Transaction();
    tx.add(
      await marketProxy.instruction.newOrderV3({
        owner: marketMakerAccounts.account.publicKey,
        payer: marketMakerAccounts.baseToken,
        side: "sell",
        price: ask[0],
        size: ask[1],
        orderType: "postOnly",
        clientId: undefined,
        openOrdersAddressKey,
        feeDiscountPubkey: null,
        selfTradeBehavior: "abortTransaction",
      })
    );
    await provider.send(tx, signers);
  }

  for (let k = 0; k < bids.length; k += 1) {
    let bid = bids[k];
    const tx = new Transaction();
    tx.add(
      await marketProxy.instruction.newOrderV3({
        owner: marketMakerAccounts.account.publicKey,
        payer: marketMakerAccounts.quoteToken,
        side: "buy",
        price: bid[0],
        size: bid[1],
        orderType: "postOnly",
        clientId: undefined,
        openOrdersAddressKey,
        feeDiscountPubkey: null,
        selfTradeBehavior: "abortTransaction",
      })
    );
    await provider.send(tx, signers);
  }
}

module.exports = {
  postOrders,
  initOpenOrders,
  KEYPAIR,
};
