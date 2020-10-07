use solana_client_gen::solana_client::rpc_client::RpcClient;

pub fn pass_time(client: &RpcClient, slot_num: u64) {
    let mut retries = 100;
    loop {
        let current_slot = client.get_slot().unwrap();
        if current_slot > slot_num {
            break;
        }
        if retries == 0 {
            assert!(false);
        }
        retries -= 1;
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}
