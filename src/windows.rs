use std::ptr;

use crate::MacchangerError;
use macaddr::MacAddr;
use windows::{
    core::{s, PCSTR, PSTR},
    Win32::{
        Foundation::{ERROR_BUFFER_OVERFLOW, ERROR_SUCCESS},
        NetworkManagement::*,
        Networking::WinSock::*,
        System::Registry::{
            RegEnumKeyExA, RegOpenKeyExA, RegQueryValueExA, HKEY, HKEY_LOCAL_MACHINE,
            KEY_ALL_ACCESS,
        },
    },
};

use IpHelper::{GetAdaptersAddresses, GAA_FLAG_INCLUDE_ALL_INTERFACES, IP_ADAPTER_ADDRESSES_LH};

pub fn change_mac_windows(_mac: MacAddr, interface: String) -> Result<(), MacchangerError> {
    let adapter = get_adapter(interface)?;
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
                            s!("DriverDesc"),
                            None,
                            None,
                            Some(value_buffer.as_mut_ptr()),
                            Some(&mut size_read),
                        )
                    };

                    if res == ERROR_SUCCESS {
                        let value = std::str::from_utf8(&value_buffer[0..(size_read - 1) as usize])
                            .map_err(|_| MacchangerError::Generic)?
                            .to_owned();
                        if value == adapter.description {
                            println!("Found correct key!");
                            break;
                        }
                    } else {
                        dbg!(res);
                        return Err(MacchangerError::Generic);
                    }
                } else {
                    dbg!(res);
                    return Err(MacchangerError::Generic);
                }
            } else {
                dbg!(res);
                return Err(MacchangerError::Generic);
            }

            cchname = 1024;
            dw_index += 1;
        }
    } else {
        dbg!(res);
        return Err(MacchangerError::Generic);
    }

    Ok(())
}

#[derive(Debug, Clone)]
struct Adapter {
    name: String,
    description: String,
    mac_address: MacAddr,
}

fn get_adapter(interface: String) -> Result<Adapter, MacchangerError> {
    let (mut adapter_list, adapter_count) = get_adapters()?;
    let original_adapter_list = adapter_list;

    loop {
        if adapter_list.is_null() {
            break;
        }

        let adapter_name = unsafe {
            (*adapter_list)
                .FriendlyName
                .to_string()
                .map_err(|_| MacchangerError::Generic)?
        };
        let adapter_description = unsafe {
            (*adapter_list)
                .Description
                .to_string()
                .map_err(|_| MacchangerError::Generic)?
        };

        if adapter_name == interface {
            let mac_bytes: [u8; 6] =
                unsafe { (*adapter_list).PhysicalAddress[..6].try_into().unwrap() };
            let mac = MacAddr::from(mac_bytes);
            return Ok(Adapter {
                name: adapter_name,
                description: adapter_description,
                mac_address: mac,
            });
        }

        adapter_list = unsafe { (*adapter_list).Next };
    }

    unsafe {
        std::alloc::dealloc(
            original_adapter_list as *mut u8,
            std::alloc::Layout::from_size_align(
                adapter_count
                    .try_into()
                    .map_err(|_| MacchangerError::Generic)?,
                core::mem::align_of::<IP_ADAPTER_ADDRESSES_LH>(),
            )
            .unwrap(),
        )
    }

    Err(MacchangerError::Generic)
}

fn get_adapters() -> Result<(*mut IP_ADAPTER_ADDRESSES_LH, u32), MacchangerError> {
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
                .unwrap(),
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
        return Err(MacchangerError::Generic);
    }

    Ok((adapter_list, buf_len))
}
