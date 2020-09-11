set -e
docker-compose up -d
solana config set --url "http://localhost:8899"
solana airdrop 10000
solana deploy --use-deprecated-loader dex/target/bpfel-unknown-unknown/release/serum_dex.so
docker ps -aqf "name=serum-dex_localnet_1" | xargs -I {} docker commit -m "Snapshot with serum dex" -p {} serum_dex_localnet:stable
docker-compose down
