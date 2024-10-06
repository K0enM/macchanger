use crate::Adapter;
use crate::Interface;
use crate::MacchangerError;
use macaddr::MacAddr;
use std::{borrow::BorrowMut, fmt::Debug, ptr, str::FromStr};

use windows::{
    core::{s, GUID, PCSTR, PSTR},
    Win32::{
        Foundation::{ERROR_BUFFER_OVERFLOW, ERROR_SUCCESS, S_FALSE, S_OK},
        NetworkManagement::*,
        Networking::WinSock::*,
        System::{
            Com::*,
            Registry::{
                RegCloseKey, RegEnumKeyExA, RegOpenKeyExA, RegQueryValueExA, RegSetValueExA, HKEY,
                HKEY_LOCAL_MACHINE, KEY_ALL_ACCESS, REG_SZ,
            },
        },
    },
};

impl From<WindowsAdapter> for Interface {
    fn from(value: WindowsAdapter) -> Self {
        Self {
            name: value.name,
            mac: value.mac_address,
        }
    }
}

impl From<WindowsAdapter> for Adapter {
    fn from(value: WindowsAdapter) -> Self {
        Self {
            name: value.description,
        }
    }
}

use IpHelper::{GetAdaptersAddresses, GAA_FLAG_INCLUDE_ALL_INTERFACES, IP_ADAPTER_ADDRESSES_LH};
use WindowsFirewall::{IEnumNetConnection, INetConnection, INetConnectionManager, NCME_DEFAULT};

pub fn change_mac(mac: MacAddr, interface: String) -> Result<MacAddr, MacchangerError> {
    let adapter = get_adapter(interface)?;
    let adapter_registry_key = get_registry_key(&adapter)?;

    let res = unsafe {
        RegSetValueExA(
            adapter_registry_key,
            s!("NetworkAddress"),
            0,
            REG_SZ,
            Some(mac.to_string().replace(":", "").as_bytes()),
        )
    };

    if res == ERROR_SUCCESS {
        change_adapter_connection_status(&adapter, false)?;
        change_adapter_connection_status(&adapter, true)?;
    } else {
        return Err(MacchangerError::ConnectionResetError);
    }
    Ok(mac)
}

pub fn get_hardware_mac(interface: String) -> Result<MacAddr, MacchangerError> {
    let adapter = get_adapter(interface)?;
    let adapter_registry_key = get_registry_key(&adapter)?;

    let mut value_buffer: [u8; 1024] = [0; 1024];
    let mut size_read: u32 = 1024;
    let res = unsafe {
        RegQueryValueExA(
            adapter_registry_key,
            s!("OriginalNetworkAddress"),
            None,
            None,
            Some(value_buffer.as_mut_ptr()),
            Some(size_read.borrow_mut()),
        )
    };

    if res != ERROR_SUCCESS {
        return Err(MacchangerError::RegistryError(res.to_hresult().message()));
    }

    let value = std::str::from_utf8(&value_buffer[0..(size_read - 1) as usize])
        .map_err(|e| MacchangerError::RegistryError(e.to_string()))?;

    let original_mac: MacAddr =
        MacAddr::from_str(value).map_err(|_| MacchangerError::StringConversionError)?;
    Ok(original_mac)
}

fn change_adapter_connection_status(
    adapter: &WindowsAdapter,
    status: bool,
) -> Result<(), MacchangerError> {
    let hr = unsafe { CoInitialize(None) };

    if hr != S_OK && hr != S_FALSE {
        return Err(MacchangerError::ConnectionResetError);
    }

    let p_net_connection_manager: INetConnectionManager;

    const CLSID_CONNECTION_MANAGER: GUID = GUID::from_u128(0xba126ad1_2166_11d1_b1d0_00805fc1270e);
    unsafe {
        match CoCreateInstance(
            &CLSID_CONNECTION_MANAGER,
            None,
            CLSCTX_LOCAL_SERVER | CLSCTX_NO_CODE_DOWNLOAD,
        ) {
            Ok(im) => p_net_connection_manager = im,
            Err(e) => {
                dbg!(e);
                return Err(MacchangerError::ConnectionResetError);
            }
        }
    };

    let p_enum_net_connection: IEnumNetConnection;
    unsafe {
        p_enum_net_connection = p_net_connection_manager
            .EnumConnections(NCME_DEFAULT)
            .map_err(|_| {
                CoUninitialize();
                MacchangerError::ConnectionResetError
            })?;
    };

    let mut fetched_count: u32 = 0;
    let mut connection_array = [const { None }; 64];

    unsafe {
        p_enum_net_connection
            .Next(&mut connection_array, &mut fetched_count)
            .map_err(|e| {
                dbg!(e);
                MacchangerError::ConnectionResetError
            })?
    };

    for c in connection_array.into_iter().flatten() {
        change_connection_status(&c, adapter, status)?
    }

    Ok(())
}

fn change_connection_status(
    connection: &INetConnection,
    adapter: &WindowsAdapter,
    status: bool,
) -> Result<(), MacchangerError> {
    let properties = unsafe { *connection.GetProperties().unwrap() };
    if (unsafe { properties.pszwDeviceName.to_string().unwrap() } == adapter.description && status)
    {
        unsafe {
            connection.Connect().map_err(|e| {
                dbg!("{}", e.message());
                MacchangerError::ConnectionResetError
            })?;
        };
        return Ok(());
    } else if (unsafe { properties.pszwDeviceName.to_string().unwrap() } == adapter.description
        && !status)
    {
        unsafe {
            connection.Disconnect().map_err(|e| {
                dbg!("{}", e.message());
                MacchangerError::ConnectionResetError
            })?
        };

        return Ok(());
    }

    Ok(())
}

pub fn get_registry_key(adapter: &WindowsAdapter) -> Result<HKEY, MacchangerError> {
    let mut main_key_handle: HKEY = HKEY(ptr::null_mut());
    let subkey: PCSTR =
        s!("SYSTEM\\CurrentControlSet\\Control\\Class\\{4d36e972-e325-11ce-bfc1-08002be10318}");
    let mut res = unsafe {
        RegOpenKeyExA(
            HKEY_LOCAL_MACHINE,
            subkey,
            0,
            KEY_ALL_ACCESS,
            &mut main_key_handle,
        )
    };

    if res == ERROR_SUCCESS {
        let mut buffer: [u8; 1024] = [0; 1024];
        let name = PSTR::from_raw(buffer.as_mut_ptr());
        let mut cchname: u32 = 1024;
        let mut dw_index: u32 = 0;
        loop {
            res = unsafe {
                RegEnumKeyExA(
                    main_key_handle,
                    dw_index,
                    name,
                    &mut cchname,
                    None,
                    PSTR::null(),
                    None,
                    None,
                )
            };

            if res == ERROR_SUCCESS {
                let mut sub_key_handle = HKEY(ptr::null_mut());
                res = unsafe {
                    RegOpenKeyExA(
                        main_key_handle,
                        name,
                        0,
                        KEY_ALL_ACCESS,
                        &mut sub_key_handle,
                    )
                };

                if res == ERROR_SUCCESS {
                    let mut value_buffer: [u8; 1024] = [0; 1024];
                    let mut size_read: u32 = 1024;

                    res = unsafe {
                        RegQueryValueExA(
                            sub_key_handle,
                            s!("NetCfgInstanceId"),
                            None,
                            None,
                            Some(value_buffer.as_mut_ptr()),
                            Some(&mut size_read),
                        )
                    };

                    if res == ERROR_SUCCESS {
                        let value = std::str::from_utf8(&value_buffer[0..(size_read - 1) as usize])
                            .map_err(|e| MacchangerError::RegistryError(e.to_string()))?
                            .to_owned();
                        if value == adapter.instance_id {
                            unsafe {
                                let _ = RegCloseKey(main_key_handle);
                            }
                            return Ok(sub_key_handle);
                        }
                    } else {
                        unsafe {
                            let _ = RegCloseKey(main_key_handle);
                            let _ = RegCloseKey(sub_key_handle);
                        }
                        return Err(MacchangerError::RegistryError(
                            "Failed to close Registry keys".to_owned(),
                        ));
                    }
                } else {
                    unsafe {
                        let _ = RegCloseKey(main_key_handle);
                        let _ = RegCloseKey(sub_key_handle);
                    }

                    return Err(MacchangerError::RegistryError(
                        "Failed to close Registry keys".to_owned(),
                    ));
                }

                unsafe {
                    let _ = RegCloseKey(sub_key_handle);
                }
            } else {
                unsafe {
                    let _ = RegCloseKey(main_key_handle);
                }

                return Err(MacchangerError::RegistryError(
                    "Failed to close Registry keys".to_owned(),
                ));
            }

            cchname = 1024;
            dw_index += 1;
        }
    } else {
        unsafe {
            let _ = RegCloseKey(main_key_handle);
        }

        Err(MacchangerError::RegistryError(
            "Failed to find necessary information in Registry".to_owned(),
        ))
    }
}

#[derive(Debug, Clone)]
pub struct WindowsAdapter {
    pub name: String,
    pub description: String,
    pub mac_address: MacAddr,
    pub instance_id: String,
}

fn get_adapter(interface: String) -> Result<WindowsAdapter, MacchangerError> {
    let adapters = list_adapters()?;

    adapters
        .iter()
        .find(|a| a.name == interface)
        .cloned()
        .ok_or(MacchangerError::Generic)
}

pub fn list_adapters() -> Result<Vec<WindowsAdapter>, MacchangerError> {
    let (mut adapter_list, adapter_count) = get_raw_adapters()?;
    let mut adapters: Vec<WindowsAdapter> = vec![];

    loop {
        if adapter_list.is_null() {
            break;
        }

        let adapter_name = unsafe {
            (*adapter_list)
                .FriendlyName
                .to_string()
                .map_err(|_| MacchangerError::AdapterError)?
        };
        let adapter_description = unsafe {
            (*adapter_list)
                .Description
                .to_string()
                .map_err(|_| MacchangerError::AdapterError)?
        };

        let adapter_instance_id = unsafe {
            (*adapter_list)
                .AdapterName
                .to_string()
                .map_err(|_| MacchangerError::AdapterError)?
        };

        let mac_bytes: [u8; 6] = unsafe {
            (*adapter_list).PhysicalAddress[..6]
                .try_into()
                .map_err(|_| MacchangerError::AdapterError)?
        };
        let mac = MacAddr::from(mac_bytes);
        adapters.push(WindowsAdapter {
            name: adapter_name,
            description: adapter_description,
            mac_address: mac,
            instance_id: adapter_instance_id,
        });

        adapter_list = unsafe { (*adapter_list).Next };
    }

    unsafe {
        std::alloc::dealloc(
            adapter_list as *mut u8,
            std::alloc::Layout::from_size_align(
                adapter_count
                    .try_into()
                    .map_err(|_| MacchangerError::Generic)?,
                core::mem::align_of::<IP_ADAPTER_ADDRESSES_LH>(),
            )
            .map_err(|_| MacchangerError::AllocError)?,
        )
    }

    Ok(adapters)
}

pub fn list_interfaces() -> Result<Vec<WindowsAdapter>, MacchangerError> {
    let (mut adapter_list, adapter_count) = get_raw_adapters()?;
    let mut adapters: Vec<WindowsAdapter> = vec![];

    loop {
        if adapter_list.is_null() {
            break;
        }

        let adapter_name = unsafe {
            (*adapter_list)
                .FriendlyName
                .to_string()
                .map_err(|_| MacchangerError::AdapterError)?
        };
        let adapter_description = unsafe {
            (*adapter_list)
                .Description
                .to_string()
                .map_err(|_| MacchangerError::AdapterError)?
        };

        let adapter_instance_id = unsafe {
            (*adapter_list)
                .AdapterName
                .to_string()
                .map_err(|_| MacchangerError::AdapterError)?
        };

        let mac_bytes: [u8; 6] = unsafe {
            (*adapter_list).PhysicalAddress[..6]
                .try_into()
                .map_err(|_| MacchangerError::AdapterError)?
        };
        let mac = MacAddr::from(mac_bytes);
        adapters.push(WindowsAdapter {
            name: adapter_name,
            description: adapter_description,
            mac_address: mac,
            instance_id: adapter_instance_id,
        });

        adapter_list = unsafe { (*adapter_list).Next };
    }

    unsafe {
        std::alloc::dealloc(
            adapter_list as *mut u8,
            std::alloc::Layout::from_size_align(
                adapter_count
                    .try_into()
                    .map_err(|_| MacchangerError::Generic)?,
                core::mem::align_of::<IP_ADAPTER_ADDRESSES_LH>(),
            )
            .map_err(|_| MacchangerError::AllocError)?,
        )
    }

    Ok(adapters)
}
fn get_raw_adapters() -> Result<(*mut IP_ADAPTER_ADDRESSES_LH, u32), MacchangerError> {
    let mut buf_len: u32 = 0;
    let mut adapter_list: *mut IP_ADAPTER_ADDRESSES_LH = &mut IP_ADAPTER_ADDRESSES_LH::default();
    let mut result: u32 = unsafe {
        GetAdaptersAddresses(
            AF_UNSPEC.0 as u32,
            GAA_FLAG_INCLUDE_ALL_INTERFACES,
            None,
            Some(adapter_list),
            &mut buf_len,
        )
    };

    if result == ERROR_BUFFER_OVERFLOW.0 {
        adapter_list = unsafe {
            std::alloc::alloc(
                std::alloc::Layout::from_size_align(
                    buf_len.try_into().map_err(|_| MacchangerError::Generic)?,
                    core::mem::align_of::<IP_ADAPTER_ADDRESSES_LH>(),
                )
                .map_err(|_| MacchangerError::AllocError)?,
            )
        } as *mut IP_ADAPTER_ADDRESSES_LH;
    }

    result = unsafe {
        GetAdaptersAddresses(
            AF_UNSPEC.0 as u32,
            GAA_FLAG_INCLUDE_ALL_INTERFACES,
            None,
            Some(adapter_list),
            &mut buf_len,
        )
    };

    if result != ERROR_SUCCESS.0 {
        return Err(MacchangerError::AdapterError);
    }

    Ok((adapter_list, buf_len))
}
