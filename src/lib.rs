mod linux;
mod windows;

use linux::change_mac_linux;
use macaddr::MacAddr;
use thiserror::Error;
use windows::change_mac_windows;

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
    UnsupportPlatform,
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
        _ => Err(MacchangerError::UnsupportPlatform),
    }
}
