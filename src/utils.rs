use crate::error::{NetError, NetResult};
use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    time::{SystemTime, UNIX_EPOCH},
};

// convert IP address to bytes
pub fn ip_to_bytes(ip: &IpAddr) -> Vec<u8> {
    match ip {
        IpAddr::V4(ipv4) => ipv4.octets().to_vec(),
        IpAddr::V6(ipv6) => ipv6.octets().to_vec(),
    }
}

// parse IP address from string
pub fn parse_ip(s: &str) -> NetResult<IpAddr> {
    s.parse()
        .map_err(|_| NetError::InvalidAddress(s.to_string()))
}

// parse IPv4 address from string
pub fn parse_ipv4(s: &str) -> NetResult<Ipv4Addr> {
    s.parse()
        .map_err(|_| NetError::InvalidAddress(s.to_string()))
}

// parse IPv6 address from string
pub fn parse_ipv6(s: &str) -> NetResult<Ipv6Addr> {
    s.parse()
        .map_err(|_| NetError::InvalidAddress(s.to_string()))
}

// get current timestamp
pub fn current_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// format bytes as human readable string
pub fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_index = 0;

    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }

    if unit_index == 0 {
        format!("{bytes} {}", UNITS[unit_index])
    } else {
        format!("{size:2} {}", UNITS[unit_index])
    }
}

// calculate network address from IP and netmask
pub fn calculate_network(ip: &Ipv4Addr, netmask: &Ipv4Addr) -> Ipv4Addr {
    let ip_int = u32::from_be_bytes(ip.octets());
    let mask_int = u32::from_be_bytes(netmask.octets());

    Ipv4Addr::from(ip_int | mask_int)
}

// calculate broadcast address from IP and netmask
pub fn calculate_broadcast(ip: &Ipv4Addr, netmask: &Ipv4Addr) -> Ipv4Addr {
    let ip_int = u32::from_be_bytes(ip.octets());
    let mask_int = u32::from_be_bytes(netmask.octets());

    Ipv4Addr::from(ip_int | !mask_int)
}

// check if IP address is in the same network
pub fn same_network(ip1: &Ipv4Addr, ip2: &Ipv4Addr, netmask: &Ipv4Addr) -> bool {
    let network1 = calculate_network(ip1, netmask);
    let network2 = calculate_network(ip2, netmask);
    network1 == network2
}

// convert CIDR notation to netmask
pub fn cidr_to_netmask(cidr: u8) -> NetResult<Ipv4Addr> {
    if cidr > 32 {
        return Err(NetError::InvalidAddress(
            "invalid CIDR notation".to_string(),
        ));
    }

    let mask = if cidr == 0 {
        0
    } else {
        u32::MAX << (32 - cidr)
    };

    Ok(Ipv4Addr::from(mask))
}

// convert netmask to CIDR notation
pub fn netmask_to_cidr(&netmask: &Ipv4Addr) -> u8 {
    let mask_int = u32::from_be_bytes(netmask.octets());
    mask_int.count_ones() as u8
}

// generate random MAC address
pub fn generate_random_mac() -> crate::ethernet::MacAddress {
    use rand::Rng;

    let mut rng = rand::thread_rng();
    let mut bytes = [0_u8; 6];
    rng.fill(&mut bytes);

    bytes[0] = (bytes[0] & 0xFE) | 0x02;

    crate::ethernet::MacAddress::new(bytes)
}

// validate MAC address string
pub fn validate_mac_address(mac: &str) -> NetResult<()> {
    let parts: Vec<&str> = mac.split(':').collect();

    if parts.len() != 6 {
        return Err(NetError::InvalidAddress(
            "MAC address must have 6 parts".to_string(),
        ));
    }

    for part in parts {
        if part.len() != 2 {
            return Err(NetError::InvalidAddress(
                "each MAC address part msut be 2 chars".to_string(),
            ));
        }

        if u8::from_str_radix(part, 16).is_err() {
            return Err(NetError::InvalidAddress(
                "Invalid hexadecimal in MAC addr".to_string(),
            ));
        }
    }

    Ok(())
}

// generate random port
pub fn generate_random_port() -> u16 {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    rng.gen_range(1024..65535)
}

// check if port is in reserved range
pub fn is_reserved_port(port: u16) -> bool {
    port < 1024
}

// parse socket address with default port
pub fn parse_socket_addr_with_default(
    addr: &str,
    default_port: u16,
) -> NetResult<std::net::SocketAddr> {
    if addr.contains(':') {
        addr.parse()
            .map_err(|_| NetError::InvalidAddress(addr.to_string()))
    } else {
        let ip: IpAddr = addr
            .parse()
            .map_err(|_| NetError::InvalidAddress(addr.to_string()))?;
        Ok(std::net::SocketAddr::new(ip, default_port))
    }
}
