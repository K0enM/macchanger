use crate::MacchangerError;
use macaddr::MacAddr;
use std::ptr;

use windows::{
    core::{s, GUID, PCSTR, PSTR},
    Win32::{
        Foundation::{ERROR_BUFFER_OVERFLOW, ERROR_SUCCESS, S_OK},
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

use IpHelper::{GetAdaptersAddresses, GAA_FLAG_INCLUDE_ALL_INTERFACES, IP_ADAPTER_ADDRESSES_LH};
use WindowsFirewall::{IEnumNetConnection, INetConnection, INetConnectionManager, NCME_DEFAULT};

pub fn change_mac_windows(mac: MacAddr, interface: String) -> Result<(), MacchangerError> {
    let adapter = get_adapter(interface)?;
    let registry_key = get_registry_key(&adapter)?;

    let res = unsafe {
        RegSetValueExA(
            registry_key,
            s!("NetworkAddress"),
            0,
            REG_SZ,
            Some(mac.to_string().as_bytes()),
        )
    };

    if res == ERROR_SUCCESS {
        change_adapter_connection_status(&adapter, false)?;
        change_adapter_connection_status(&adapter, true)?;
    } else {
        return Err(MacchangerError::ConnectionResetError);
    }
    Ok(())
}

fn change_adapter_connection_status(
    adapter: &Adapter,
    status: bool,
) -> Result<(), MacchangerError> {
    let hr = unsafe { CoInitialize(None) };

    if hr != S_OK {
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

    for c in connection_array {
        if let Some(c) = c {
            change_connection_status(&c, adapter, status)?
        }
    }

    Ok(())
}

fn change_connection_status(
    connection: &INetConnection,
    adapter: &Adapter,
    status: bool,
) -> Result<(), MacchangerError> {
    let properties = unsafe { *connection.GetProperties().unwrap() };
    dbg!(unsafe { properties.pszwDeviceName.to_string().unwrap() });
    if (unsafe { properties.pszwDeviceName.to_string().unwrap() } == adapter.description && status)
    {
        unsafe {
            loop {
                let res = connection.Connect().map_err(|e| {
                    println!("{}", e.message());
                    MacchangerError::ConnectionResetError
                });
                if res.is_ok() {
                    break;
                }
            }
        };
        return Ok(());
    } else if (unsafe { properties.pszwDeviceName.to_string().unwrap() } == adapter.description
        && !status)
    {
        unsafe {
            connection.Disconnect().map_err(|e| {
                println!("{}", e.message());
                MacchangerError::ConnectionResetError
            })?
        };

        return Ok(());
    }

    Ok(())
}

pub fn get_registry_key(adapter: &Adapter) -> Result<HKEY, MacchangerError> {
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
                            .map_err(|_| MacchangerError::RegistryError)?
                            .to_owned();
                        if value == adapter.instance_id {
                            unsafe {
                                RegCloseKey(main_key_handle);
                            }
                            return Ok(sub_key_handle);
                        }
                    } else {
                        dbg!(res);

                        unsafe {
                            RegCloseKey(main_key_handle);
                            RegCloseKey(sub_key_handle);
                        }
                        return Err(MacchangerError::RegistryError);
                    }
                } else {
                    dbg!(res);

                    unsafe {
                        RegCloseKey(main_key_handle);
                        RegCloseKey(sub_key_handle);
                    }
                    return Err(MacchangerError::RegistryError);
                }

                unsafe {
                    RegCloseKey(sub_key_handle);
                }
            } else {
                dbg!(res);

                unsafe {
                    RegCloseKey(main_key_handle);
                }
                return Err(MacchangerError::RegistryError);
            }

            cchname = 1024;
            dw_index += 1;
        }
    } else {
        dbg!(res);
        unsafe {
            RegCloseKey(main_key_handle);
        }
        Err(MacchangerError::RegistryError)
    }
}

#[derive(Debug, Clone)]
pub struct Adapter {
    pub name: String,
    pub description: String,
    pub mac_address: MacAddr,
    pub instance_id: String,
    real_adapter: PhysicalAdapter,
}

#[derive(Clone)]
struct PhysicalAdapter {
    inner: IP_ADAPTER_ADDRESSES_LH,
}

impl std::fmt::Debug for PhysicalAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(unsafe { &self.inner.FriendlyName.to_string().unwrap() })
    }
}

fn get_adapter(interface: String) -> Result<Adapter, MacchangerError> {
    let adapters = get_adapters()?;

    adapters
        .iter()
        .find(|a| a.name == interface)
        .cloned()
        .ok_or(MacchangerError::Generic)
}

pub fn get_adapters() -> Result<Vec<Adapter>, MacchangerError> {
    let (mut adapter_list, adapter_count) = get_raw_adapters()?;
    let mut adapters: Vec<Adapter> = vec![];

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
        adapters.push(Adapter {
            name: adapter_name,
            description: adapter_description,
            mac_address: mac,
            instance_id: adapter_instance_id,
            real_adapter: PhysicalAdapter {
                inner: unsafe { *adapter_list },
            },
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
