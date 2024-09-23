mod linux;
mod util;
mod windows;

use linux::change_mac_linux;
use macaddr::MacAddr;
use thiserror::Error;
pub use util::generate_random_mac;
use windows::{change_mac_windows, get_adapters, Adapter};

#[derive(Debug, Clone, Copy)]
enum MacchangerPlatform {
    Linux,
    Windows,
}

#[derive(Error, Debug)]
pub enum MacchangerError {
    #[error("Generic error")]
    Generic,
    #[error("This platform is not supported")]
    UnsupportedPlatform,
    #[error("Something went wrong when interacting with the registry")]
    RegistryError,
    #[error("Something went wrong when converting to/from a rust String")]
    StringConversionError,
    #[error("Something went wrong when disabling/enabling the relevant adapter")]
    ConnectionResetError,
    #[error("Something went wrong when trying to allocate memory for the adapter list")]
    AllocError,
    #[error("Something went wrong when working with the adapter list")]
    AdapterError,
}

pub fn change_mac(mac: MacAddr, interface: String) -> Result<(), MacchangerError> {
    let platform = check_platform()?;
    match platform {
        MacchangerPlatform::Linux => change_mac_linux(mac, interface),
        MacchangerPlatform::Windows => change_mac_windows(mac, interface),
    }
}

fn check_platform() -> Result<MacchangerPlatform, MacchangerError> {
    let os = std::env::consts::OS;
    match os {
        "linux" => Ok(MacchangerPlatform::Linux),
        "windows" => Ok(MacchangerPlatform::Windows),
        _ => Err(MacchangerError::UnsupportedPlatform),
    }
}

#[derive(Debug, Clone)]
pub struct Interface {
    pub name: String,
    pub mac: MacAddr,
}

impl From<Adapter> for Interface {
    fn from(value: Adapter) -> Self {
        Interface {
            name: value.name,
            mac: value.mac_address,
        }
    }
}

pub fn list_interfaces() -> Result<Vec<Interface>, MacchangerError> {
    let platform = check_platform()?;
    match platform {
        MacchangerPlatform::Linux => todo!(),
        MacchangerPlatform::Windows => {
            let adapters: Vec<Interface> = get_adapters()?
                .into_iter()
                .map(Interface::from)
                .collect();
            Ok(adapters)
        }
    }
}
