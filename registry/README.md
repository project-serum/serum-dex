# Serum Registry

For a technical introduction to the Registry and its role in facilitating staking,
see [Serum Staking](../docs/staking.md).

## Developing

Because, the Registry builds and composes multiple programs, `make` is used to simplify
development.

### Build

```
make build
```

### Run unit tests

```
make test-unit
```

### Run integration tests

```
make test
```

These integration tests assume a localnetwork is running at `http://localhost:8899`
and a wallet is funded and located at `~/.config/solana/id.json`.

## Deploying

To deploy and initialize the entire staking and lockup system to a network, e.g., Devnet,
`cd ../` into the repository's top level directory and run

```
./scripts/deploy-staking.sh devnet
```

This script will deploy and initialize all required programs and accounts required for a
[UI](https://github.com/project-serum/serum-ts/tree/master/packages/stake-ui) to be functional,
outputting generated TypeScript that can be copy and pasted into any program that needs
the newly deployed and intialized addresses, for example, this network [configuration](https://github.com/project-serum/serum-ts/blob/master/packages/common/src/networks.ts#L44).
