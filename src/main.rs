use anyhow::Result;
use clap::{Parser, Subcommand};
use macaddr::MacAddr;
use macchanger_lib::{
    do_change_mac, generate_random_mac, retrieve_adapters, retrieve_hardware_mac,
    retrieve_interfaces,
};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
#[command(propagate_version = true)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Change {
        interface: String,
        mac: Option<MacAddr>,
    },
    ListInterfaces,
    ListAdapters,
    ListMacs,
    Restore {
        interface: String,
    },
}

fn main() -> Result<()> {
    let args = Args::parse();

    match &args.command {
        Commands::ListInterfaces => {
            let interfaces = retrieve_interfaces()?;
            println!("Found {} interfaces", interfaces.len());
            for interface in interfaces {
                println!("{}", interface.name);
            }
        }
        Commands::ListMacs => {
            let interfaces = retrieve_interfaces()?;
            println!("Found {} MAC addresses", interfaces.len());
            for interface in interfaces {
                println!(
                    "Interface: {} - MAC address: {}",
                    interface.name, interface.mac
                );
            }
        }
        Commands::ListAdapters => {
            let adapters = retrieve_adapters()?;
            println!("Found {} adapters", adapters.len());
            for adapter in adapters {
                println!("{}", adapter.name);
            }
        }
        Commands::Change { interface, mac } => {
            let mac = match mac {
                Some(mac) => *mac,
                None => generate_random_mac(),
            };
            let current_mac = do_change_mac(mac, interface.clone())?;
            println!(
                "Successfully changed MAC address of interface {} to {}",
                interface, current_mac
            );
        }
        Commands::Restore { interface } => {
            let original_mac = retrieve_hardware_mac(interface.clone())?;
            println!("Found original (hardware) MAC address of {}", original_mac);
            let current_mac = do_change_mac(original_mac, interface.clone())?;
            println!(
                "Successfully changed MAC address of interface {} to {}",
                interface, current_mac
            );
        }
    }

    Ok(())
}
