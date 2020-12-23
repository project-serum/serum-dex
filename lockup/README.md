# Serum Lockup

For a technical introduction to the Lockup program see [Serum Lockup](../docs/lockup.md).

## Developing

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
