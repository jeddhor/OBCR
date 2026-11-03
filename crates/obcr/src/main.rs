//! OBCR: a modern source port of Operation Body Count.

use clap::Parser;

#[derive(Parser)]
#[command(version, about)]
struct Cli {}

fn main() -> anyhow::Result<()> {
    let _cli = Cli::parse();
    println!("obcr {}", env!("CARGO_PKG_VERSION"));
    Ok(())
}
