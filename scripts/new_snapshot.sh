set -e
localnet_url="http://localhost:8899"
docker-compose up -d
solana airdrop -u $localnet_url 10
solana deploy -u $localnet_url dex/target/bpfel-unknown-unknown/release/serum_dex.so
docker ps -aqf "name=serum-dex_localnet_1" | xargs -I {} docker commit -m "Snapshot with serum dex" -p {} serum_dex_localnet:edge
docker-compose down
