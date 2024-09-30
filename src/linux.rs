use crate::MacchangerError;
use macaddr::MacAddr;
use nix::ifaddrs::{getifaddrs, InterfaceAddress};
use pci_ids::Device;
use std::{fs, ops::ControlFlow, path::Path};
use thiserror::Error;

pub fn change_mac_linux(_mac: MacAddr, interface: String) -> Result<MacAddr, MacchangerError> {
    let interfaces = list_interfaces_linux()?;
    let _interface: Option<&LinuxInterface> = interfaces
        .into_iter()
        .filter(|i| i.name == interface)
        .collect::<Vec<LinuxInterface>>()
        .first();
    todo!()
}

#[derive(Debug, Clone)]
pub struct LinuxInterface {
    pub name: String,
    pub adapter: LinuxAdapter,
}

#[derive(Debug, Error)]
pub enum LinuxMacchangerError {
    #[error("Something went wrong with getting the SockaddrStorage from the interface")]
    SocketaddrStorage,
    #[error("Something went wrong with getting the LinkAddr from the SockaddrStorage")]
    LinkAddress,
    #[error("Something went wrong with getting the MacAddres from the LinkAddr")]
    MacAddressBytes,
    #[error("Something went wrong with looking up the name of the network adapter: {0}")]
    AdapterNameLookup(String),
}

impl From<LinuxMacchangerError> for MacchangerError {
    fn from(val: LinuxMacchangerError) -> Self {
        MacchangerError::LinuxError(val)
    }
}

struct OptionalLinuxInterface(Option<LinuxInterface>);

impl TryFrom<InterfaceAddress> for OptionalLinuxInterface {
    type Error = LinuxMacchangerError;

    fn try_from(interface: InterfaceAddress) -> Result<Self, Self::Error> {
        if interface.address.is_none() {
            return Ok(OptionalLinuxInterface(None));
        }

        let socket_address = match &interface.address {
            Some(address) => *address,
            None => return Err(LinuxMacchangerError::SocketaddrStorage),
        };

        if socket_address.as_link_addr().is_none() {
            return Ok(OptionalLinuxInterface(None));
        }

        let link_address = socket_address.as_link_addr().unwrap();
        if link_address.addr().unwrap().iter().all(|b| *b == 0) {
            return Ok(OptionalLinuxInterface(None));
        }
        let mac = match link_address.addr() {
            Some(address_bytes) => MacAddr::from(address_bytes),
            None => return Err(LinuxMacchangerError::MacAddressBytes),
        };

        let vendor_path = format!("/sys/class/net/{}/device/vendor", &interface.interface_name);
        let device_path = format!("/sys/class/net/{}/device/device", &interface.interface_name);

        let adapter_name = if !Path::new(&vendor_path).exists() || !Path::new(&device_path).exists()
        {
            "Virtual Adapter (non-existent)"
        } else {
            let vendor_string = fs::read_to_string(vendor_path)
                .map_err(|e| LinuxMacchangerError::AdapterNameLookup(e.to_string()))?;

            let vendor_string = vendor_string.trim_end();
            let vendor_string = vendor_string.strip_prefix("0x").unwrap();

            let vendor_id = u16::from_str_radix(vendor_string, 16)
                .map_err(|e| LinuxMacchangerError::AdapterNameLookup(e.to_string()))?;

            let device_string = fs::read_to_string(device_path)
                .map_err(|e| LinuxMacchangerError::AdapterNameLookup(e.to_string()))?;
            let device_string = device_string.trim_end();
            let device_string = device_string.strip_prefix("0x").unwrap();
            let device_id = u16::from_str_radix(device_string, 16)
                .map_err(|e| LinuxMacchangerError::AdapterNameLookup(e.to_string()))?;

            let device = Device::from_vid_pid(vendor_id, device_id).ok_or(
                LinuxMacchangerError::AdapterNameLookup(
                    "Could not find network adapter by vendor id and device id".to_owned(),
                ),
            )?;
            device.name()
        };
        let interface = LinuxInterface {
            name: interface.clone().interface_name,
            adapter: LinuxAdapter {
                name: adapter_name.to_owned(),
                mac,
            },
        };

        Ok(OptionalLinuxInterface(Some(interface)))
    }
}

#[derive(Debug, Clone)]
pub struct LinuxAdapter {
    pub name: String,
    pub mac: MacAddr,
}

pub fn list_interfaces_linux() -> Result<Vec<LinuxInterface>, MacchangerError> {
    let mut addrs = getifaddrs().map_err(|_| MacchangerError::ListInterfacesError)?;
    let mut interfaces: Vec<LinuxInterface> = vec![];
    let r = addrs.try_for_each(|i| {
        let interface_result = OptionalLinuxInterface::try_from(i);
        if interface_result.is_err() {
            ControlFlow::Break(interface_result)
        } else {
            let optional_interface = interface_result.unwrap();
            if let Some(interface) = optional_interface.0 {
                interfaces.push(interface);
            }

            ControlFlow::Continue(())
        }
    });
    if let ControlFlow::Break(Err(e)) = r {
        Err(MacchangerError::LinuxError(e))
    } else {
        Ok(interfaces)
    }
}

pub fn list_adapters_linux() -> Result<Vec<LinuxAdapter>, MacchangerError> {
    let mut addrs = getifaddrs().map_err(|_| MacchangerError::ListInterfacesError)?;
    let mut adapters: Vec<LinuxAdapter> = vec![];
    let r = addrs.try_for_each(|i| {
        let interface_result = OptionalLinuxInterface::try_from(i);
        if interface_result.is_err() {
            ControlFlow::Break(interface_result)
        } else {
            let optional_interface = interface_result.unwrap();
            if let Some(interface) = optional_interface.0 {
                adapters.push(interface.adapter);
            }

            ControlFlow::Continue(())
        }
    });
    if let ControlFlow::Break(Err(e)) = r {
        Err(MacchangerError::LinuxError(e))
    } else {
        Ok(adapters)
    }
}

pub fn get_hardware_mac_linux(_interface: String) -> Result<MacAddr, MacchangerError> {
    todo!()
}
