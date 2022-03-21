use anyhow::Result;
use clap::Parser;
use crank::Opts;

fn main() -> Result<()> {
    let opts = Opts::parse();
    crank::start(opts)
}
