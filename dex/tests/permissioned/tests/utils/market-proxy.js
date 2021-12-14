const anchor = require("@project-serum/anchor");
const {
  PublicKey,
  SYSVAR_RENT_PUBKEY,
  SYSVAR_CLOCK_PUBKEY,
} = require("@solana/web3.js");
const {
  OpenOrders,
  OpenOrdersPda,
  Logger,
  ReferralFees,
  PermissionedCrank,
  MarketProxyBuilder,
} = require("@project-serum/serum");

// Returns a client for the market proxy.
//
// If changing the program, one will likely need to change the builder/middleware
// here as well.
async function load(connection, proxyProgramId, dexProgramId, market) {
  return new MarketProxyBuilder()
    .middleware(
      new OpenOrdersPda({
        proxyProgramId,
        dexProgramId,
      })
    )
    .middleware(new ReferralFees())
    .middleware(new Identity())
    .middleware(new Logger())
    .load({
      connection,
      market,
      dexProgramId,
      proxyProgramId,
      options: { commitment: "recent" },
    });
}

// Dummy identity middleware used for testing.
class Identity {
  initOpenOrders(ix) {
    this.proxy(ix);
  }
  newOrderV3(ix) {
    this.proxy(ix);
  }
  cancelOrderV2(ix) {
    this.proxy(ix);
  }
  cancelOrderByClientIdV2(ix) {
    this.proxy(ix);
  }
  settleFunds(ix) {
    this.proxy(ix);
  }
  closeOpenOrders(ix) {
    this.proxy(ix);
  }
  prune(ix) {
    this.proxyRevoked(ix);
  }
  proxy(ix) {
    ix.keys = [
      { pubkey: SYSVAR_RENT_PUBKEY, isWritable: false, isSigner: false },
      ...ix.keys,
    ];
  }
  proxyRevoked(ix) {
    ix.keys = [
      { pubkey: SYSVAR_CLOCK_PUBKEY, isWritable: false, isSigner: false },
      ...ix.keys,
    ];
  }
  consumeEventsPermissioned(ix) {
    ix.keys = [
      { pubkey: SYSVAR_CLOCK_PUBKEY, isWritable: false, isSigner: false },
      ...ix.keys,
    ];
    // PDA: so ensure the signer is false.
    ix.keys[ix.keys.length-1].isSigner = false;
  }
  static async pruneAuthority(market, dexProgramId, proxyProgramId) {
    const [addr] = await PublicKey.findProgramAddress(
      [
        anchor.utils.bytes.utf8.encode("prune"),
        dexProgramId.toBuffer(),
        market.toBuffer(),
      ],
      proxyProgramId
    );
    return addr;
  }

  static async consumeEventsAuthority(market, dexProgramId, proxyProgramId) {
    return Identity.pruneAuthority(market, dexProgramId, proxyProgramId);
  }
}

module.exports = {
  load,
  Identity,
};
