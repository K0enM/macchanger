use crate::MacchangerError;
use macaddr::MacAddr;
use nix::{
    errno::Errno,
    ifaddrs::{getifaddrs, InterfaceAddress},
    ioctl_read_bad, ioctl_readwrite_bad,
    libc::{
        sockaddr, ARPHRD_ETHER, IFF_UP, IF_NAMESIZE, SIOCETHTOOL, SIOCGIFFLAGS, SIOCSIFFLAGS,
        SIOCSIFHWADDR,
    },
    sys::socket::{socket, SockFlag},
};
use pci_ids::Device;
use std::{
    fs,
    io::Write,
    ops::ControlFlow,
    os::fd::{AsRawFd, OwnedFd},
    path::Path,
};
use thiserror::Error;

pub fn change_mac(mac: MacAddr, interface: String) -> Result<MacAddr, MacchangerError> {
    // temp(&interface)?;
    let interfaces = list_interfaces()?;
    let temp_interface = interfaces
        .into_iter()
        .filter(|i| i.name == interface)
        .collect::<Vec<LinuxInterface>>();
    let interface = temp_interface.first().unwrap();
    let socket = get_socket()?;
    let mut mac_bytes_i8: [i8; 14] = [0; 14];
    for (i, b) in mac.as_bytes().iter().enumerate() {
        mac_bytes_i8[i] = *b as i8;
    }

    let status = change_interface_active(interface, false)?;
    assert!(!status);

    let mut req = IfreqAddress::from(interface);
    // Set the new MAC address bytes
    req.value.sa_data = mac_bytes_i8;
    ioctl_readwrite_bad!(set_mac_address, SIOCSIFHWADDR, IfreqAddress);
    let _ = unsafe {
        set_mac_address(socket.as_raw_fd(), &mut req)
            .map_err(LinuxMacchangerError::SetMacAddress)?
    };

    let status = change_interface_active(interface, true)?;
    assert!(status);
    Ok(mac)
}

fn change_interface_active(
    interface: &LinuxInterface,
    active: bool,
) -> Result<bool, MacchangerError> {
    let socket = get_socket()?;
    let mut req = IfreqFlags::from(interface);
    ioctl_read_bad!(get_ifr_flags, SIOCGIFFLAGS, IfreqFlags);
    ioctl_readwrite_bad!(set_ifr_flags, SIOCSIFFLAGS, IfreqFlags);
    let _ = unsafe {
        get_ifr_flags(socket.as_raw_fd(), &mut req).map_err(LinuxMacchangerError::GetIfrFlags)?
    };
    match active {
        false => {
            req.value &= !IFF_UP as u16;
        }
        true => {
            req.value |= IFF_UP as u16;
        }
    };
    let _ = unsafe {
        set_ifr_flags(socket.as_raw_fd(), &mut req).map_err(LinuxMacchangerError::SetIfrFlags)?
    };
    let status = req.value & IFF_UP as u16 == 1;
    Ok(status)
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
struct IfreqAddress {
    name: [u8; IF_NAMESIZE],
    value: sockaddr,
}

impl From<&LinuxInterface> for IfreqAddress {
    fn from(interface: &LinuxInterface) -> Self {
        let mut req = IfreqAddress {
            name: [0; IF_NAMESIZE],
            value: sockaddr {
                sa_family: ARPHRD_ETHER,
                sa_data: [0; 14],
            },
        };
        let mut mac_bytes_i8: [i8; 14] = [0; 14];
        for (i, b) in interface.adapter.mac.as_bytes().iter().enumerate() {
            mac_bytes_i8[i] = *b as i8;
        }
        req.name
            .as_mut()
            .write_all(interface.name.as_bytes())
            .map_err(|e| LinuxMacchangerError::CreateIfreqAddress(e.to_string()))
            .unwrap();
        req
    }
}

#[derive(Debug, Clone, Copy)]
struct IfreqFlags {
    name: [u8; IF_NAMESIZE],
    value: u16,
}

impl From<&LinuxInterface> for IfreqFlags {
    fn from(interface: &LinuxInterface) -> Self {
        let mut req = IfreqFlags {
            name: [0; IF_NAMESIZE],
            value: 0,
        };

        req.name
            .as_mut()
            .write_all(interface.name.as_bytes())
            .map_err(|e| LinuxMacchangerError::CreateIfreqAddress(e.to_string()))
            .unwrap();
        req
    }
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
    #[error("Something went wrong with getting the MacAddress from the LinkAddr")]
    MacAddressBytes,
    #[error("Something went wrong with looking up the name of the network adapter: {0}")]
    AdapterNameLookup(String),
    #[error("Something went wrong with retrieving a socket: {0}")]
    GetSocket(Errno),
    #[error("Something went wrong with creating the IfreqAddress struct: {0}")]
    CreateIfreqAddress(String),
    #[error("Something went wrong with setting the MacAddress: {0}")]
    SetMacAddress(Errno),
    #[error("Something went wrong with getting the ifr_flags: {0}")]
    GetIfrFlags(Errno),
    #[error("Something went worng with setting the ifr_flags: {0}")]
    SetIfrFlags(Errno),
    #[error("Something went wrong with getting the permanent MacAddress: {0}")]
    GetPermanentMac(Errno),
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

pub fn list_interfaces() -> Result<Vec<LinuxInterface>, MacchangerError> {
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

pub fn list_adapters() -> Result<Vec<LinuxAdapter>, MacchangerError> {
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

const MAX_ADDR_LEN: u32 = 32;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct EthtoolRequest {
    cmd: u32,
    size: u32,
    data: [u8; MAX_ADDR_LEN as usize],
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct IfreqEthtool {
    name: [u8; IF_NAMESIZE],
    value: *mut EthtoolRequest,
}

pub fn get_hardware_mac(interface: String) -> Result<MacAddr, MacchangerError> {
    let interfaces = list_interfaces()?;
    let temp_interface = interfaces
        .into_iter()
        .filter(|i| i.name == interface)
        .collect::<Vec<LinuxInterface>>();
    let interface = temp_interface.first().unwrap();
    let socket = get_socket()?;
    let mut epa = EthtoolRequest {
        cmd: 0x00000020,
        size: MAX_ADDR_LEN,
        data: [0u8; MAX_ADDR_LEN as usize],
    };
    let mut req = IfreqEthtool {
        name: [0u8; IF_NAMESIZE],
        value: &mut epa,
    };
    req.name
        .as_mut()
        .write_all(interface.name.as_bytes())
        .map_err(|e| LinuxMacchangerError::CreateIfreqAddress(e.to_string()))
        .unwrap();

    ioctl_read_bad!(get_permanent_mac_address, SIOCETHTOOL, IfreqEthtool);
    let _ = unsafe {
        get_permanent_mac_address(socket.as_raw_fd(), &mut req)
            .map_err(LinuxMacchangerError::GetPermanentMac)?
    };

    match epa.size {
        6 => {
            let mac_data: [u8; 6] = epa.data[0..6].try_into().unwrap();
            Ok(MacAddr::from(mac_data))
        }
        8 => {
            let mac_data: [u8; 8] = epa.data[0..6].try_into().unwrap();
            Ok(MacAddr::from(mac_data))
        }
        _ => Err(MacchangerError::LinuxError(
            LinuxMacchangerError::MacAddressBytes,
        )),
    }
}

fn get_socket() -> Result<OwnedFd, LinuxMacchangerError> {
    let res = socket(
        nix::sys::socket::AddressFamily::Inet,
        nix::sys::socket::SockType::Datagram,
        SockFlag::empty(),
        Some(nix::sys::socket::SockProtocol::Udp),
    )
    .map_err(LinuxMacchangerError::GetSocket)?;
    Ok(res)
}
