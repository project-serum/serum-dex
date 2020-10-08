use clap::Clap;
use serum_node::Config;

fn main() {
    let cfg = Config::parse();
    let handle = serum_node::start(cfg);
    handle.park();
}
