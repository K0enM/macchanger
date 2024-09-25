use macaddr::MacAddr;
use rand::Rng;

pub fn generate_random_mac() -> MacAddr {
    let mut rng = rand::thread_rng();
    let mut mac = [0u8; 6];

    rng.fill(&mut mac);

    mac[0] = 0x02;

    MacAddr::from(mac)
}
