use std::ptr;

use macaddr::{MacAddr, MacAddr8};
use IpHelper::{GetAdaptersAddresses, GAA_FLAG_INCLUDE_ALL_INTERFACES, IP_ADAPTER_ADDRESSES_LH};
use crate::MacchangerError;
use windows::Win32::{Foundation::{ERROR_BUFFER_OVERFLOW, ERROR_SUCCESS, WIN32_ERROR}, NetworkManagement::*, Networking::WinSock::*};

pub fn change_mac_windows(mac: MacAddr, interface: String) -> Result<(), MacchangerError> {
  let mac = get_mac(interface)?;
  dbg!(mac.to_string());
  Ok(())
}

fn get_mac(interface: String) -> Result<MacAddr, MacchangerError> {
  let (mut adapter_list, adapter_count) = get_adapters()?;
  let original_adapter_list = adapter_list;

  loop {
    if adapter_list.is_null() {
      break
    }

    let adapter_name = unsafe { (*adapter_list).FriendlyName.to_string().map_err(|_| MacchangerError::Generic)?};

    if adapter_name == interface {
      let mac_bytes: [u8;6] = unsafe {
        (*adapter_list).PhysicalAddress[..6].try_into().unwrap()
      };
      let mac = MacAddr::from(mac_bytes);
      return Ok(mac)
    }

    adapter_list = unsafe { (*adapter_list).Next };
  }

  unsafe { 
    std::alloc::dealloc(
      original_adapter_list as *mut u8,
  std::alloc::Layout::from_size_align(
      adapter_count.try_into().map_err(|_| MacchangerError::Generic)?, 
      core::mem::align_of::<IP_ADAPTER_ADDRESSES_LH>()
      ).unwrap()
    ) 
  }
  todo!()
}

fn get_adapters() -> Result<(*mut IP_ADAPTER_ADDRESSES_LH, u32), MacchangerError>{
  let mut buf_len: u32 = 0;
  let mut adapter_list: *mut IP_ADAPTER_ADDRESSES_LH = &mut IP_ADAPTER_ADDRESSES_LH::default();
  let mut result: u32 = unsafe  {
    GetAdaptersAddresses(
      AF_UNSPEC.0 as u32, 
      GAA_FLAG_INCLUDE_ALL_INTERFACES, 
      None, 
      Some(adapter_list), 
      &mut buf_len
    )
  };

  if result == ERROR_BUFFER_OVERFLOW.0 {
    adapter_list = unsafe {
      std::alloc::alloc(
        std::alloc::Layout::from_size_align(
          buf_len.try_into().map_err(|_| MacchangerError::Generic)?, 
          core::mem::align_of::<IP_ADAPTER_ADDRESSES_LH>()
        ).unwrap()
      )
    } as *mut IP_ADAPTER_ADDRESSES_LH;
  }

  result = unsafe  {
    GetAdaptersAddresses(
      AF_UNSPEC.0 as u32, 
      GAA_FLAG_INCLUDE_ALL_INTERFACES, 
      None, 
      Some(adapter_list), 
      &mut buf_len
    )
  };

  if result != ERROR_SUCCESS.0 {
    return Err(MacchangerError::Generic)
  } 

  Ok((adapter_list, buf_len))
}