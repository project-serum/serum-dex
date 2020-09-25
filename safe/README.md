# Serum Safe

⚠️ **WARNING**: Serum Safe is a work in progress and is not audited. Do not use in production.

## Developing

### Dependencies

For the canonical list of dependencies to build the project, see the
[Dockerfile](https://github.com/project-serum/serum-dex/blob/armani/fmt/docker/development/Dockerfile).

### Commands

A set of commands for developing can be found in the [Makefile](./Makefile). Some useful
ones include

#### Build

To build run

```
make build
```

#### Deploy

To deploy run

```
make deploy
```

Deploy assumes

* a Solana local network is running with it's JSON RPC exposed on at `http://localhost:8899`
* a funded keypair is found at `~/.config/solana.json`

#### Test

To run all the tests run

```
make test
```

Test makes the same assumptions as `make deploy`.

#### Clippy

It's recommended to run clippy before submitting changes.

```
make clippy
```
