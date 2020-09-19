mod common;

#[test]
fn solana() {
    let client = common::client();
    let sig = serum_common::rpc::create_account_rent_exempt(
        client.rpc(),
        client.payer(),
        1, //10_000_000,
        client.program(),
    )
    .unwrap();
    println!("test {:?}", sig);
}
