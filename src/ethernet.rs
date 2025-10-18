use crate::error::{NetError, NetResult};
use crate::packet::{Packet, PacketBuilder};
use std::fmt;

// ethernet frame types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum EtherType {
    // hex number base-16
    // 0,1,2,3,4,5,6,7,8,9,A,B,C,D,E,F
    //
    // binary :1111111 1111111 dst
    // hex: FF:FF:FF:FF:FF:FF
    // human: "Broadcast MAC Address"
    //
    // conversion:
    // F = 1111
    // A = 1010
    // 0 = 0000
    Ipv4 = 0x0800,
    Ipv6 = 0x86DD,
    Arp = 0x0806,
    Unknown(u16),
}

// application layer | hello, how are you? | (your message)
// transport layer | TCP/UDP (amplop) | (services)
// internet layer | IP address (amplop) | (global addr)
// data link layer | ETHERNET FRAME | (local delivery)
// physical layer | Wire | (actual transmision)

impl From<u16> for EtherType {
    fn from(value: u16) -> Self {
        match value {
            0x0800 => EtherType::Ipv4,
            0x86DD => EtherType::Ipv6,
            0x0806 => EtherType::Arp,
            other => EtherType::Unknown(other),
        }
    }
}

impl From<EtherType> for u16 {
    fn from(ether_type: EtherType) -> Self {
        match ether_type {
            EtherType::Ipv4 => 0x0800,
            EtherType::Ipv6 => 0x86DD,
            EtherType::Arp => 0x0806,
            EtherType::Unknown(value) => value,
        }
    }
}

// MAC address representation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MacAddress([u8; 6]); // ab:cd:ef:gh:ij:kl
// 6 bytes (48 bits) long

impl MacAddress {
    pub const BROADCAST: MacAddress = MacAddress([0xFF; 6]); // 0xFF = 255 in decimal = 111111111 in binary
    pub const ZERO: MacAddress = MacAddress([0x00; 6]);

    pub fn new(bytes: [u8; 6]) -> Self {
        MacAddress(bytes)
    }

    pub fn from_bytes(bytes: &[u8]) -> NetResult<Self> {
        if bytes.len() != 6 {
            return Err(NetError::packet_parse("invalid MAC address length"));
        }

        let mut addr = [0_u8; 6];
        addr.copy_from_slice(bytes);
        Ok(MacAddress(addr))
    }

    pub fn as_bytes(&self) -> &[u8; 6] {
        &self.0
    }

    pub fn is_broadcast(&self) -> bool {
        *self == Self::BROADCAST
    }

    pub fn is_multicast(&self) -> bool {
        self.0[0] & 0x01 != 0
    }

    pub fn is_unicast(&self) -> bool {
        !self.is_multicast()
    }
}

impl fmt::Display for MacAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            self.0[0], self.0[0], self.0[0], self.0[0], self.0[0], self.0[0]
        )
    }
}

impl std::str::FromStr for MacAddress {
    type Err = NetError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() != 6 {
            return Err(NetError::packet_parse("Invalid MAC address format"));
        }

        let mut bytes = [0_u8; 6];
        for (i, part) in parts.iter().enumerate() {
            bytes[i] = u8::from_str_radix(part, 16)
                .map_err(|_| NetError::packet_parse("invalid MAC address hex"))?;
        }

        Ok(MacAddress(bytes))
    }
}

// ethernet frame header
#[derive(Debug, Clone)]
pub struct EthernetHeader {
    pub destination: MacAddress,
    pub source: MacAddress,
    pub ether_type: EtherType,
}

impl EthernetHeader {
    pub const LENGTH: usize = 14; // 6 + 6 + 2 bytes

    pub fn new(destination: MacAddress, source: MacAddress, ether_type: EtherType) -> Self {
        Self {
            destination,
            source,
            ether_type,
        }
    }

    pub fn parse(packet: &mut Packet) -> NetResult<Self> {
        if packet.len() < Self::LENGTH {
            return Err(NetError::packet_parse("ethernet frame too short"));
        }

        let dest_bytes = packet.read_bytes(6)?;
        let src_bytes = packet.read_bytes(6)?;
        let ether_type = packet.read_u16()?;

        Ok(EthernetHeader {
            destination: MacAddress::from_bytes(&dest_bytes)?,
            source: MacAddress::from_bytes(&src_bytes)?,
            ether_type: EtherType::from(ether_type),
        })
    }

    pub fn serialize(&self, builder: &mut PacketBuilder) {
        builder.write_bytes(self.destination.as_bytes());
        builder.write_bytes(self.source.as_bytes());
        builder.write_u16(self.ether_type.into());
    }
}

// the ethernet frame
#[derive(Debug, Clone)]
pub struct EthernetFrame {
    pub header: EthernetHeader,
    pub payload: Packet,
}

impl EthernetFrame {
    pub fn new(header: EthernetHeader, payload: Packet) -> Self {
        Self { header, payload }
    }

    pub fn parse(mut packet: Packet) -> NetResult<Self> {
        let header = EthernetHeader::parse(&mut packet)?;

        Ok(EthernetFrame {
            header,
            payload: packet,
        })
    }

    pub fn serialize(&self) -> Packet {
        let mut builder = PacketBuilder::with_capacity(EthernetHeader::LENGTH + self.payload.len());

        self.header.serialize(&mut builder);
        builder.write_bytes(self.payload.as_slice());

        builder.build()
    }
}
