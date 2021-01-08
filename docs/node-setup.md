# Serum Node Setup

WARNING: All code related to Serum Nodes is unaudited. Use at your own risk.

## Introduction

Serum nodes are run by staked node leaders, who become eligible for cranking
when their node has at least 1 MSRM staked. These "cranking rewards"
are effectively transaction fees earned for operating the DEX.

For an introduction to the DEX and the idea of cranking, see
[A technical introduction to the Serum DEX](https://docs.google.com/document/d/1isGJES4jzQutI0GtQGuqtrBUqeHxl_xJNXdtOv4SdII/edit). For an introduction to staking, see [Serum Staking](./staking.md).

The way cranking rewards work is simple, instead of sending transactions directly to the DEX,
a cranker sends transactions to a cranking rewards vendor, which is an on-chain
Solana program that proxies all requests to the DEX, recording the amount of events
cranked, and then sends a reward to the cranker's wallet as a function of the number
of events processed and the reward vendor's fee rate. This proxy program can be found [here](../registry/rewards/program).

If the rewards vendor's vault becomes empty or if the node leader's Entity stake
balance ever transitions to **inactive**, then the vendor will refuse to pay
rewards to the node leader until the vault is funded and/or the node becomes **active** again.

## Install Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

On Linux systems you may need to install additional dependencies. On Ubuntu,

```bash
sudo apt-get install -y pkg-config build-essential python3-pip jq
```

## Install the CLI

For now, we can use Cargo.

```bash
cargo install --git https://github.com/project-serum/serum-dex serum-cli --locked
```

To verify the installation worked, run `serum -h`.

## Setup your CLI Config

Add your YAML config for Devnet at `~/.config/serum/cli/config.yaml`.

```yaml
---
network:
  cluster: devnet

mints:
  srm: Bap9SwT53SjGPeKq4LAC6i86fCzEzUGGHsYfRiCFSGyF
  msrm: CmtL8e86367ZLiAuJELx4WqmDz7dRnD1oyaiq4TQDdEU

programs:
  rewards_pid: 7sXyzeu6GJqkXZz8VhjdsXvDg1xR1PEkXbbDaxMc186C
  registry_pid: CKKz2WYvneiLb2mzouWc4iPpKisuXs5XKYn7ZUrRjkeK
  meta_entity_pid: BsVgsh8mqi3qn8eKRiye1a4eF8Qwqot8p2n3ZMNdL2UY
  lockup_pid: 8wreDpv5nuY1gee1X4wkqtRkzoGypVYzWBrMmzipAJKN
  dex_pid: 9MVDeYQnJmN2Dt7H44Z8cob4bET2ysdNu2uFJcatDJno

accounts:
  registrar: 7Tzf4D4BU1tzwitXbHeUf7bwMNSSVQXfzPsgnbM5RY7d
  safe: CxiGCt8kVm5BuzmWycJdZwXsYt52iLB8Huty5r1xRvRZ
  rewards_instance: 32qFc9QWX4wBU6EFZ9FzyxGthSHNwCUKaN7APn3GAH2X
```

If not specified, the `wallet` key will be searched for in the standard location:
`~/.config/solana/id.json` and used as the **payer** for all transactions initiated
by the CLI.

## Create an Entity

An **Entity** is the on-chain Solana account representing a node and
it's collective **Member** accounts.

To create an **Entity**  with the **Registrar**, run

```bash
serum registry create-entity --name <string> --about <string>
```

Entering your node's `name` and `about` info, which can be displayed in UIs. Note that, by default,
the wallet creating the entity will be tagged as the node leader, which is the address eligible for
earning crank rewards.

## Create a Member

After creating a node entity, use your new **Entity** address to create a member account, which will
allow you to stake.

```bash
serum registry create-member --entity <address>
```

## Activate your Node

Once created, one must "activate" a node by staking MSRM before being able to earn rewards. Any **Member**
associated with the **Entity** can stake this MSRM.

For now, it's recommended to do this through the UI at https://project-serum.github.io/serum-ts/stake-ui,
where you can

* Select a network.
* Connect your wallet.
* Deposit 1 MSRM into your **Member** account via the **Deposit** button. If you're on Devnet,
  [airdrop](https://www.spl-token-ui.com/#/token-faucets) yourself tokens. The faucet address can be found in the **Environment** tab of the UI.
* Stake the newly deposited 1 MSRM via the **Stake** tab into the **Mega Stake Pool**.

You should see that your Entity is now in the `Active` state making it eligible for rewards.

## Cranking a Market

Finally you can run your crank. Pick a market and run

```bash
serum crank consume-event-rewards \
  --market <address>  \
  --log-directory <path> \
  --rewards.receiver <address> \
  --rewards.registry-entity <address>
```

If the given `--rewards.registry-entity` is properly staked, and if the configured
rewards `instance` is funded, then you should see your SPL token account
`--rewards.receiver` start to receive rewards with each event consumed.

## Finding a Market to Crank

You can crank any market of your choosing. To find all market addresses one can use the `getProgramAccounts`
API exposed by the Solana JSON RPC. In python,

```python
def find_market_addresses(program_id: str):
    resp = requests.post('https://devnet.solana.com', json={
        'jsonrpc': '2.0',
        'method': 'getProgramAccounts',
        'id': 1,
        'params': [
            program_id,
            {
                'encoding': 'base64',
                'filters': [
                    # Base58 encoding of 0x0300000000000000
                    {'memcmp': {'offset': 5, 'bytes': 'W723RTUpoZ'}},
                ],
            },
        ],
    }).json()
    return [info['pubkey'] for info in resp['result']]
```

## Running your own Solana Validator

Cranking markets can place a lot of load on a Solana RPC server. As a result, it's highly recommended
to run one's own validator. Otherwise, one is subject to any rate limits imposed by a given RPC node.
An example and guide for setting up your own Solana validator can be found [here](https://github.com/project-serum/validators). Once setup is complete, one can specify its url with the `network.cluster` option in the CLI config.

## Switching to Mainnet Beta

When operating over multiple networks, you can specify your config file with the
`serum --config <path>` option. For example, when switching to Mainnet Beta,
one can use the following config.

```yaml
---
network:
  cluster: mainnet

mints:
  srm: SRMuApVNdxXokk5GT7XD5cUUgXMBCoAz2LHeuAoKWRt
  msrm: MSRMcoVyrFxnSgo5uXwone5SKcGhT1KEJMFEkMEWf9L

programs:
  rewards_pid: 4bcHoAgLP9NBje1oVo9WKRDYkvSxcqtJeTSXMRFX5AdZ
  registry_pid: 6J7ZoSxtKJUjVLpGRcBrEtvE2T3YVf9mfKUaicndzpCc
  meta_entity_pid: 68gpi9be8NNVTDViQxSYtbM1788uebczX2Vz7obSnQRz
  lockup_pid: 4nvqpaMz7H12VgHSABjEDFmH62MoWP3BxfMG3BAFQiBo
  dex_pid: EUqojwWA2rd19FZrzeBncJsm38Jm1hEhE3zsmX3bRc2o

accounts:
  registrar: 8ZS85GGfa92JH6vrmpy8fgQQEBLWwYUwWmpBhcA94fDH
  safe: Dp5zzdTLnYNq9E6H81YQu1ucNLK3FzH3Ah3KedydiNgE
  rewards_instance: BX1aNRFES78bz9G8GJWmrYSkRVWcV5wdnvSWfXJhTEkE
```
