# Serum Node Setup

WARNING: All code related to Serum Nodes is unaudited. Use at your own risk.

## Introduction

Serum nodes are run by staked node leaders, who become eligible for cranking
when their node has at least 1 MSRM staked. These "cranking rewards"
are effectively transaction fees earned for operating the DEX.

For an introduction to the DEX and the idea of cranking, see
[A technical introduction to the Serum DEX](https://docs.google.com/document/d/1isGJES4jzQutI0GtQGuqtrBUqeHxl_xJNXdtOv4SdII/edit).

The way cranking rewards work is simple, instead of sending transactions directly to the DEX,
a cranker sends transactions to a cranking rewards vendor, which is an on-chain
Solana program that proxies all requests to the DEX, recording the amount of events
cranked, and then sends a reward to the cranker's wallet as a function of the number
of events processed and the reward vendor's fee rate.

(Note that, although similar in spirit, the cranking rewards vendor is an entirely different
program and account from the **Registry**'s reward vendors. Only node leaders are eligible
to crank.)

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

The CLI is a work in progress, so there's not yet a proper installer.
For now, we can use Cargo.

```bash
cargo install --git https://github.com/project-serum/serum-dex serum-cli
```

To verify the installation worked, run `serum -h`.

## Setup your CLI Config

Add your YAML config for Mainnet Beta at `~/.config/serum/cli/config.yaml`.

For Mainnet Beta

```yaml
---
network:
  cluster: mainnet

mints:
  srm: SRMuApVNdxXokk5GT7XD5cUUgXMBCoAz2LHeuAoKWRt
  msrm: MSRMcoVyrFxnSgo5uXwone5SKcGhT1KEJMFEkMEWf9L

programs:
  rewards_pid: 8xYo1X6uw7SBngXgPzib8jghWb8BhiiVxv5yV799Tw3G
  registry_pid: Gw1XNGbSnx7PJcHTTuxxhWfkjjPmq29Qkv1hWbVFnrDp
  meta_entity_pid: 9etE5ZjHZTrZ2wQfyfTSp5WBxjpvaakNJa5fSVToZn17
  lockup_pid: 6GSn1woRF541HaiEWqNofYn8quzJuRBPi1nwoho8zNnh
  dex_pid: EUqojwWA2rd19FZrzeBncJsm38Jm1hEhE3zsmX3bRc2o
```

When operating over multiple networks, you can specify your config file with the
`serum --config <path>` option. For example, one might want to test
against Devnet with the following config

```yaml
---
network:
  cluster: devnet

mints:
  srm: 5ya5rnzm5MkvCXhLDCJqAUzT16A37ks2DekkfoNnwNMn
  msrm: 3fHm9sEBS3CukUX366mwzn3YwEc5zWNzR4JcGK6EVQad

programs:
  rewards_pid: EXzpf5GBfUQkwLeLEJXLmVKxGpxyMQWxpudYxogW4ad8
  registry_pid: 3ofaHrxu7RdqH8m1wXfVrsTqgwctmx2NsHPv6m7oh1dy
  meta_entity_pid: 8v8hwdeyBhmV4y235F9XQ7g5Vz2EYvJTkGqTfrh3Hz5f
  lockup_pid: Az4dD6YeA4akzz4Qx3RuQqaCtLEaDiBT8u7mDL24sbAu
  dex_pid: DiDDva9iDSXTtJ4CeWXbKdDvQ3M6g5G87PZGUGvxi3eV
```

## Cranking a market

Finally you can run your crank. Pick a market and run

```bash
  serum crank consume-event-rewards \
    --market <address>  \
    --log-directory <path> \
    --rewards.receiver <address> \
    --rewards.registry-entity <address> \
    --rewards.instance <address>
```

If the given `--rewards.registry-entity` is properly staked, and if the given
`--rewards.instance` is funded, then you should see your token account
`--rewards.receiver` start to receive rewards with each event consumed.

## Finding a market to crank

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
