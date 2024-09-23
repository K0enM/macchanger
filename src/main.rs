use anyhow::Result;
use clap::{Parser, Subcommand};
use macaddr::MacAddr;
use macchanger_lib::{change_mac, generate_random_mac, list_interfaces};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
#[command(propagate_version = true)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    ListInterfaces,
    ListAdapters,
    ListMacs,
    Change {
        interface: String,
        mac: Option<MacAddr>,
    },
}

fn main() -> Result<()> {
    let args = Args::parse();

    match &args.command {
        Commands::ListInterfaces => {
            let interfaces = list_interfaces()?;
            println!("Found {} interfaces", interfaces.len());
            for i in interfaces {
                println!("{}", i.name);
            }
        }
        Commands::ListMacs => {
            let interfaces = list_interfaces()?;
            println!("Found {} MAC addresses", interfaces.len());
            for i in interfaces {
                println!("Interface: {}, MAC address: {}", i.name, i.mac);
            }
        }
        Commands::ListAdapters => todo!(),
        Commands::Change { interface, mac } => {
            let mac = match mac {
                Some(mac) => *mac,
                None => generate_random_mac(),
            };
            change_mac(mac, interface.clone())?;
        }
    }

    Ok(())
}
