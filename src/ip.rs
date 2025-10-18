// IP Protocol implementation (IPv4 and IPv6)

use crate::error::{NetError, NetResult};
use crate::packet::{Packet, PacketBuilder, internet_checksum};
use std::net::{Ipv4Addr, Ipv6Addr};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum IpProtocol {
    Icmp = 1,
    Tcp = 6,
    Udp = 17,
    Icmpv6 = 50,
    Unknown(u8),
}

impl From<u8> for IpProtocol {
    fn from(value: u8) -> Self {
        match value {
            1 => IpProtocol::Icmp,
            6 => IpProtocol::Tcp,
            17 => IpProtocol::Udp,
            50 => IpProtocol::Icmpv6,
            other => IpProtocol::Unknown(other),
        }
    }
}

impl From<IpProtocol> for u8 {
    fn from(protocol: IpProtocol) -> Self {
        match protocol {
            IpProtocol::Icmp => 1,
            IpProtocol::Tcp => 6,
            IpProtocol::Udp => 17,
            IpProtocol::Icmpv6 => 50,
            IpProtocol::Unknown(value) => value,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Ipv4Header {
    pub version: u8,
    pub header_length: u8,
    pub type_of_service: u8,
    pub total_length: u16,
    pub identification: u16,
    pub flags: u8,
    pub fragment_offset: u16,
    pub time_to_live: u8,
    pub protocol: IpProtocol,
    pub checksum: u16,
    pub source: Ipv4Addr,
    pub destination: Ipv4Addr,
    pub options: Vec<u8>,
}

impl Ipv4Header {
    pub const MIN_LENGTH: usize = 20;
    pub const MAX_LENGTH: usize = 60;

    pub fn new(source: Ipv4Addr, destination: Ipv4Addr, protocol: IpProtocol) -> Self {
        Self {
            version: 4,
            header_length: 5, // 20 bytes
            type_of_service: 0,
            total_length: 0,
            identification: rand::random(),
            flags: 0x40, // do not fragment
            fragment_offset: 0,
            time_to_live: 64,
            protocol,
            checksum: 0,
            source,
            destination,
            options: Vec::new(),
        }
    }

    pub fn parse(packet: &mut Packet) -> NetResult<Self> {
        if packet.len() < Self::MIN_LENGTH {
            return Err(NetError::packet_parse("IPv4 header too short"));
        }

        let version_ihl = packet.read_u8()?;
        let version = version_ihl >> 4;
        let header_length = version_ihl & 0x0F;

        if version != 4 {
            return Err(NetError::protocol("IPv4", "Invalid version"));
        }
        if header_length < 5 {
            return Err(NetError::protocol("IPv4", "Header length too small"));
        }

        let type_of_service = packet.read_u8()?;
        let total_length = packet.read_u16()?;
        let identification = packet.read_u16()?;
        let flags_fragment = packet.read_u16()?;
        let flags = (flags_fragment >> 13) as u8;
        let fragment_offset = flags_fragment & 0x1FFF;
        let time_to_live = packet.read_u8()?;
        let protocol = IpProtocol::from(packet.read_u8()?);
        let checksum = packet.read_u16()?;

        let source_bytes = packet.read_bytes(4)?;
        let destination_bytes = packet.read_bytes(4)?;

        let source = Ipv4Addr::from([
            source_bytes[0],
            source_bytes[1],
            source_bytes[2],
            source_bytes[3],
        ]);

        let destination = Ipv4Addr::from([
            destination_bytes[0],
            destination_bytes[1],
            destination_bytes[2],
            destination_bytes[3],
        ]);

        let options_length = (header_length as usize - 5) * 4;
        let options = if options_length > 0 {
            packet.read_bytes(options_length)?.to_vec()
        } else {
            Vec::new()
        };

        Ok(Ipv4Header {
            version,
            header_length,
            type_of_service,
            total_length,
            identification,
            flags,
            fragment_offset,
            time_to_live,
            protocol,
            checksum,
            source,
            destination,
            options,
        })
    }

    pub fn serialize(&mut self, payload_length: usize) -> Vec<u8> {
        self.total_length = (self.header_length as usize * 4 + payload_length) as u16;

        let mut buffer = Vec::with_capacity(self.header_length as usize * 4);

        buffer.push((self.version << 4) | self.header_length);
        buffer.push(self.type_of_service);
        buffer.extend_from_slice(&self.total_length.to_be_bytes());
        buffer.extend_from_slice(&self.identification.to_be_bytes());

        let flags_fragment = ((self.flags as u16) << 13) | self.fragment_offset;
        buffer.extend_from_slice(&flags_fragment.to_be_bytes());

        buffer.push(self.time_to_live);
        buffer.push(self.protocol.into());
        buffer.extend_from_slice(&[0, 0]);
        buffer.extend_from_slice(&self.source.octets());
        buffer.extend_from_slice(&self.destination.octets());
        buffer.extend_from_slice(&self.options);

        // calculate and insert checksum
        let checksum = internet_checksum(&buffer);
        buffer[10] = (checksum >> 8) as u8;
        buffer[11] = checksum as u8;

        buffer
    }

    pub fn is_fragmented(&self) -> bool {
        (self.flags & 0x01) != 0 || self.fragment_offset != 0
    }
}

#[derive(Debug, Clone)]
pub struct Ipv6Header {
    pub version: u8,
    pub traffic_class: u8,
    pub flow_label: u32,
    pub payload_length: u16,
    pub next_header: IpProtocol,
    pub hop_limit: u8,
    pub source: Ipv6Addr,
    pub destination: Ipv6Addr,
}

impl Ipv6Header {
    pub const LENGTH: usize = 40;

    pub fn new(source: Ipv6Addr, destination: Ipv6Addr, next_header: IpProtocol) -> Self {
        Self {
            version: 6,
            traffic_class: 0,
            flow_label: 0,
            payload_length: 0,
            next_header,
            hop_limit: 64,
            source,
            destination,
        }
    }

    pub fn parse(packet: &mut Packet) -> NetResult<Self> {
        if packet.len() < Self::LENGTH {
            return Err(NetError::packet_parse("IPv6 header too short"));
        }

        let version_tc_fl = packet.read_u32()?;
        let version = (version_tc_fl >> 20) as u8;
        let traffic_class = ((version_tc_fl >> 20) & 0xFF) as u8;
        let flow_label = version_tc_fl & 0xFFFFF;

        if version != 6 {
            return Err(NetError::protocol("IPv6", "Invalid version"));
        }

        let payload_length = packet.read_u16()?;
        let next_header = IpProtocol::from(packet.read_u8()?);
        let hop_limit = packet.read_u8()?;

        let source_bytes = packet.read_bytes(16)?;
        let destination_bytes = packet.read_bytes(16)?;

        let mut source_addr = [0_u8; 16];
        let mut dest_addr = [0_u8; 16];
        source_addr.copy_from_slice(&source_bytes);
        dest_addr.copy_from_slice(&destination_bytes);

        Ok(Ipv6Header {
            version,
            traffic_class,
            flow_label,
            payload_length,
            next_header,
            hop_limit,
            source: Ipv6Addr::from(source_addr),
            destination: Ipv6Addr::from(dest_addr),
        })
    }

    pub fn serialize(&mut self, payload_length: usize) -> Vec<u8> {
        self.payload_length = payload_length as u16;

        let mut buffer = Vec::with_capacity(Self::LENGTH);

        let version_tc_fl =
            ((self.version as u32) << 28) | ((self.traffic_class as u32) << 20) | self.flow_label;

        buffer.extend_from_slice(&version_tc_fl.to_be_bytes());

        buffer.extend_from_slice(&self.payload_length.to_be_bytes());
        buffer.push(self.next_header.into());
        buffer.push(self.hop_limit);
        buffer.extend_from_slice(&self.source.octets());
        buffer.extend_from_slice(&self.destination.octets());

        buffer
    }
}

#[derive(Debug, Clone)]
pub enum IpPacket {
    V4 { header: Ipv4Header, payload: Packet },
    V6 { header: Ipv6Header, payload: Packet },
}

impl IpPacket {
    pub fn parse_v4(mut packet: Packet) -> NetResult<Self> {
        let header = Ipv4Header::parse(&mut packet)?;
        Ok(IpPacket::V4 {
            header,
            payload: packet,
        })
    }
    pub fn parse_v6(mut packet: Packet) -> NetResult<Self> {
        let header = Ipv6Header::parse(&mut packet)?;
        Ok(IpPacket::V6 {
            header,
            payload: packet,
        })
    }

    pub fn protocol(&self) -> IpProtocol {
        match self {
            IpPacket::V4 { header, .. } => header.protocol,
            IpPacket::V6 { header, .. } => header.next_header,
        }
    }

    pub fn payload(&self) -> &Packet {
        match self {
            IpPacket::V4 { payload, .. } => payload,
            IpPacket::V6 { payload, .. } => payload,
        }
    }

    pub fn serialize(&mut self) -> Packet {
        match self {
            IpPacket::V4 { header, payload } => {
                let header_bytes = header.serialize(payload.len());
                let mut builder = PacketBuilder::with_capacity(header_bytes.len() + payload.len());
                builder.write_bytes(&header_bytes);
                builder.write_bytes(payload.as_slice());
                builder.build()
            }
            IpPacket::V6 { header, payload } => {
                let header_bytes = header.serialize(payload.len());
                let mut builder = PacketBuilder::with_capacity(header_bytes.len() + payload.len());
                builder.write_bytes(&header_bytes);
                builder.write_bytes(payload.as_slice());
                builder.build()
            }
        }
    }
}
