# serum-dex

Docker Image: https://hub.docker.com/r/michaelhly/serum_dex_localnet

ProgramId: `35xWbYPmt7hzyjsmJ9m3hZekN63mmeQDTygd7i8zudiy`

## Run unit tests
```
./do.sh test dex
```

## Compile the dex binary
```
./do.sh build dex
```

## Deploy the dex

Install the Solana Tool Suite:
```
curl -sSf https://raw.githubusercontent.com/solana-labs/solana/v1.3.9/install/solana-install-init.sh | sh -s - v1.3.9
```

## To pre-configured solana cluster
```
DEX_PROGRAM_ID="$(solana deploy dex/target/bpfel-unknown-unknown/release/serum_dex.so | jq .programId -r)"
```
## To local network
1. Install [docker](https://www.docker.com/)
2. Generate Keypair
```
solana-keygen new -o ~/.config/solana/id.json
```
3. Start docker container
```
docker-compose up
```
4. Airdrop SOL to yourself
```
solana airdrop -u http://localhost:8899 10000
```
5. Deploy the dex to your localnet instance
```
solana deploy -u http://localhost:8899 dex/target/bpfel-unknown-unknown/release/serum_dex.so
```

## Using the client utility
```
cd crank
cargo run -- help
```
