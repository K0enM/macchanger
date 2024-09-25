use crate::MacchangerError;
use macaddr::MacAddr;

pub fn change_mac_linux(_mac: MacAddr, _interface: String) -> Result<MacAddr, MacchangerError> {
    todo!()
}
