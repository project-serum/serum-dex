<div align="center">
  <img height="170" src="http://github.com/project-serum/awesome-serum/blob/master/logo-serum.png?raw=true" />

  <h1>serum-dex</h1>

  <p>
    <strong>Project Serum Rust Monorepo</strong>
  </p>

  <p>
    <a href="https://travis-ci.com/project-serum/serum-dex"><img alt="Build Status" src="https://travis-ci.com/project-serum/serum-dex.svg?branch=master" /></a>
    <a href="https://discord.com/channels/739225212658122886"><img alt="Discord Chat" src="https://img.shields.io/discord/739225212658122886?color=blueviolet" /></a>
    <a href="https://opensource.org/licenses/Apache-2.0"><img alt="License" src="https://img.shields.io/github/license/project-serum/serum-dex?color=blue" /></a>
  </p>

  <h4>
    <a href="https://projectserum.com/">Website</a>
    <span> | </span>
    <a href="https://serum-academy.com/en/">Academy</a>
    <span> | </span>
    <a href="https://github.com/project-serum/awesome-serum">Awesome</a>
    <span> | </span>
    <a href="https://dex.projectserum.com/#/">DEX</a>
    <span> | </span>
    <a href="https://github.com/project-serum/serum-ts">TypeScript</a>
  </h4>
</div>

## Program Deployments

| Program | Devnet | Mainnet Beta |
| --------|--------|------------- |
| [DEX](/dex)     | `F9b23Ph1JdBev2fULXTZLzaxVh2nYVdMVq9CTEaEZrid` | `EUqojwWA2rd19FZrzeBncJsm38Jm1hEhE3zsmX3bRc2o` |
| [Registry](/registry/program) | `FigXetJcXogqm94qfmyKWy6U5KJAwtxSgJMjUHercVQp` | `Gw1XNGbSnx7PJcHTTuxxhWfkjjPmq29Qkv1hWbVFnrDp` |
| [Lockup](/lockup/program) | `CiNaYvdnQ42BNdbKvvAapHxiP18pvc3Vk5WuZ59ia64x` | `6GSn1woRF541HaiEWqNofYn8quzJuRBPi1nwoho8zNnh` |
| [Crank Rewards](/registry/rewards/program) | `EXzpf5GBfUQkwLeLEJXLmVKxGpxyMQWxpudYxogW4ad8` | `8xYo1X6uw7SBngXgPzib8jghWb8BhiiVxv5yV799Tw3G`|

## Note

* **Serum is in active development so all APIs and protocols are subject to change.**
* **The code is unaudited. Use at your own risk.**

## Contributing

### Install Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
rustup component add rustfmt
```

On Linux systems you may need to install additional dependencies. On Ubuntu,

```bash
sudo apt-get install -y pkg-config build-essential python3-pip jq
```

### Install Solana

```bash
curl -sSf https://raw.githubusercontent.com/solana-labs/solana/v1.4.14/install/solana-install-init.sh | sh -s - v1.4.14
export PATH="/home/ubuntu/.local/share/solana/install/active_release/bin:$PATH"
```

### Download the source

```bash
git clone https://github.com/project-serum/serum-dex.git
```

### Install the BPF SDK

```bash
./do.sh update
```

### Build, deploy, and test programs

See individual crates for documentation. For example, to build the dex see its [README](https://github.com/project-serum/serum-dex/tree/armani/readme/dex).

## Running a local Solana cluster

The easiest way to run a local cluster is to run the docker container provided by Solana.
Instructions can be found [here](https://solana-labs.github.io/solana-web3.js/). For local development, however, it's often convenient to build and run a validator from [source](https://github.com/solana-labs/solana#building).

## Directories

* `assert-owner`: Solana utility program for checking account ownership.
* `cli`: Serum command line interface.
* `common`: Common rust utilities.
* `context`: Global environment used by Serum crates, read from a configuration file.
* `dex`: Serum DEX program and client utility.
* `docker`: Docker image definitions.
* `lockup`: Serum Lockup program and clients.
* `pool`: Serum pool protocol.
* `registry`: Serum staking registry and client.
* `scripts`: Bash scripts for development.
* `solana-client-gen`: Proc macro for generating Rust clients from instruction definitions.
