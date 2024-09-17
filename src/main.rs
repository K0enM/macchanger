use std::str::FromStr;

use anyhow::Result;
use clap::Parser;
use macaddr::MacAddr;
use macchanger_lib::change_mac;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    interface: String,

    #[arg(short, long)]
    mac: Option<MacAddr>,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let interface = args.interface;
    let mac = match args.mac {
        Some(mac) => mac,
        None => MacAddr::from_str("00:0E:F6:E0:35:60").unwrap(),
    };

    change_mac(mac, interface)?;
    Ok(())
}
