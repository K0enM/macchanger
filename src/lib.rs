mod linux;
mod util;
#[cfg(target_os = "windows")]
mod windows;

use linux::{
    change_mac_linux, get_hardware_mac_linux, list_adapters_linux, list_interfaces_linux,
    LinuxAdapter, LinuxInterface, LinuxMacchangerError,
};
use macaddr::MacAddr;
use thiserror::Error;
pub use util::generate_random_mac;
#[cfg(target_os = "windows")]
use windows::get_hardware_mac_windows;
#[cfg(target_os = "windows")]
use windows::{change_mac_windows, get_adapters, WindowsAdapter};

#[derive(Debug, Clone, Copy)]
enum MacchangerPlatform {
    Linux,
    #[cfg(target_os = "windows")]
    Windows,
}

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

pub fn change_mac(mac: MacAddr, interface: String) -> Result<MacAddr, MacchangerError> {
    let platform = check_platform()?;
    match platform {
        MacchangerPlatform::Linux => change_mac_linux(mac, interface),
        #[cfg(target_os = "windows")]
        MacchangerPlatform::Windows => change_mac_windows(mac, interface),
    }
}

fn check_platform() -> Result<MacchangerPlatform, MacchangerError> {
    let os = std::env::consts::OS;
    match os {
        "linux" => Ok(MacchangerPlatform::Linux),
        #[cfg(target_os = "windows")]
        "windows" => Ok(MacchangerPlatform::Windows),
        _ => Err(MacchangerError::UnsupportedPlatform),
    }
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
pub fn list_interfaces() -> Result<Vec<Interface>, MacchangerError> {
    let platform = check_platform()?;
    match platform {
        MacchangerPlatform::Linux => {
            let interfaces = list_interfaces_linux()?
                .into_iter()
                .map(Interface::from)
                .collect();
            Ok(interfaces)
        }
        #[cfg(target_os = "windows")]
        MacchangerPlatform::Windows => {
            let adapters: Vec<Interface> =
                get_adapters()?.into_iter().map(Interface::from).collect();
            Ok(adapters)
        }
    }
}

pub fn list_adapters() -> Result<Vec<Adapter>, MacchangerError> {
    let platform = check_platform()?;
    match platform {
        MacchangerPlatform::Linux => {
            let adapters = list_adapters_linux()?
                .into_iter()
                .map(Adapter::from)
                .collect();
            Ok(adapters)
        }
        #[cfg(target_os = "windows")]
        MacchangerPlatform::Windows => {
            let adapters = get_adapters()?;
            Ok(adapters)
        }
    }
}

pub fn get_hardware_mac(interface: String) -> Result<MacAddr, MacchangerError> {
    let platform = check_platform()?;
    match platform {
        MacchangerPlatform::Linux => get_hardware_mac_linux(interface),
        #[cfg(target_os = "windows")]
        MacchangerPlatform::Windows => get_hardware_mac_windows(interface),
    }
}
