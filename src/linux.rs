use macaddr::MacAddr;
use crate::MacchangerError;

pub fn change_mac_linux(_mac: MacAddr, _interface: String) -> Result<(), MacchangerError> {
  todo!()
}