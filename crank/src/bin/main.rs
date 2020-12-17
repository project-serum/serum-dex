use anyhow::Result;
use clap::Clap;
use crank::Opts;

fn main() -> Result<()> {
    let opts = Opts::parse();
    crank::start(None, opts)
}
