#[cfg(target_os = "windows")]
#[path("windows.rs")]
mod os;

mod util;

#[cfg(target_os = "linux")]
#[path = "linux.rs"]
mod os;

use macaddr::MacAddr;
use os::{
    change_mac, get_hardware_mac, list_adapters, list_interfaces, LinuxAdapter, LinuxInterface,
    LinuxMacchangerError,
};
use thiserror::Error;
pub use util::generate_random_mac;

#[derive(Error, Debug)]
pub enum MacchangerError {
    #[error("Generic error")]
    Generic,
    #[error("This platform is not supported")]
    UnsupportedPlatform,
    #[error("Something went wrong when interacting with the registry: {0}")]
    RegistryError(String),
    #[error("Something went wrong when converting to/from a rust String")]
    StringConversionError,
    #[error("Something went wrong when disabling/enabling the relevant adapter")]
    ConnectionResetError,
    #[error("Something went wrong when trying to allocate memory for the adapter list")]
    AllocError,
    #[error("Something went wrong when working with the adapter list")]
    AdapterError,
    #[error("Something went wrong when retrieving the interface list")]
    ListInterfacesError,
    #[error("Something went wrong with the Linux code: {0}")]
    LinuxError(LinuxMacchangerError),
}

pub fn do_change_mac(mac: MacAddr, interface: String) -> Result<MacAddr, MacchangerError> {
    change_mac(mac, interface)
}

#[derive(Debug, Clone)]
pub struct Interface {
    pub name: String,
    pub mac: MacAddr,
}

#[cfg(target_os = "windows")]
impl From<WindowsAdapter> for Interface {
    fn from(value: WindowsAdapter) -> Self {
        Interface {
            name: value.name,
            mac: value.mac_address,
        }
    }
}

impl From<LinuxInterface> for Interface {
    fn from(value: LinuxInterface) -> Self {
        Interface {
            name: value.name,
            mac: value.adapter.mac,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Adapter {
    pub name: String,
}

#[cfg(target_os = "windows")]
impl From<WindowsAdapter> for Adapter {
    fn from(value: WindowsAdapter) -> Self {
        Adapter {
            name: value.description,
        }
    }
}

impl From<LinuxAdapter> for Adapter {
    fn from(value: LinuxAdapter) -> Self {
        Adapter { name: value.name }
    }
}
pub fn retrieve_interfaces() -> Result<Vec<Interface>, MacchangerError> {
    let interfaces = list_interfaces()?
        .into_iter()
        .map(Interface::from)
        .collect();
    Ok(interfaces)
}

pub fn retrieve_adapters() -> Result<Vec<Adapter>, MacchangerError> {
    let adapters = list_adapters()?.into_iter().map(Adapter::from).collect();
    Ok(adapters)
}

pub fn retrieve_hardware_mac(interface: String) -> Result<MacAddr, MacchangerError> {
    get_hardware_mac(interface)
}
