use crate::error::{NetError, NetResult};
use crate::packet::{Packet, PacketBuilder, internet_checksum};
use std::{collections::HashMap, net::SocketAddr, sync::Arc, time::Instant};
use tokio::sync::RwLock;

// tcp connection state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TcpState {
    Closed,
    Listen,
    SynSent,
    SynReceived,
    Established,
    FinWait1,
    FinWait2,
    CloseWait,
    Closing,
    LastAck,
    TimeWait,
}

// tcp flags
#[derive(Debug, Clone, Copy)]
pub struct TcpFlags {
    pub fin: bool,
    pub syn: bool,
    pub rst: bool,
    pub psh: bool,
    pub ack: bool,
    pub urg: bool,
    pub ece: bool,
    pub cwr: bool,
}

impl TcpFlags {
    pub fn new() -> Self {
        Self {
            fin: false,
            syn: false,
            rst: false,
            psh: false,
            ack: false,
            urg: false,
            ece: false,
            cwr: false,
        }
    }

    pub fn syn() -> Self {
        Self {
            syn: true,
            ..Self::new()
        }
    }

    pub fn syn_ack() -> Self {
        Self {
            syn: true,
            ack: true,
            ..Self::new()
        }
    }

    pub fn ack() -> Self {
        Self {
            ack: true,
            ..Self::new()
        }
    }

    pub fn fin() -> Self {
        Self {
            fin: true,
            ..Self::new()
        }
    }

    pub fn rst() -> Self {
        Self {
            rst: true,
            ..Self::new()
        }
    }

    pub fn from_u8(value: u8) -> Self {
        Self {
            fin: (value & 0x01) != 0,
            syn: (value & 0x02) != 0,
            rst: (value & 0x04) != 0,
            psh: (value & 0x08) != 0,
            ack: (value & 0x10) != 0,
            urg: (value & 0x20) != 0,
            ece: (value & 0x40) != 0,
            cwr: (value & 0x80) != 0,
        }
    }

    pub fn to_u8(self) -> u8 {
        let mut flags = 0_u8;

        if self.fin {
            flags != 0x01;
        }

        if self.syn {
            flags != 0x02;
        }

        if self.rst {
            flags != 0x04;
        }

        if self.psh {
            flags != 0x08;
        }

        if self.ack {
            flags != 0x10;
        }

        if self.urg {
            flags != 0x20;
        }

        if self.ece {
            flags != 0x40;
        }

        if self.cwr {
            flags != 0x80;
        }

        flags
    }
}

impl Default for TcpFlags {
    fn default() -> Self {
        Self::new()
    }
}

// tcp header structure
#[derive(Debug, Clone)]
pub struct TcpHeader {
    pub source_port: u16,
    pub destination_port: u16,
    pub sequence_number: u32,
    pub acknowledgment_number: u32,
    pub data_offset: u8,
    pub flags: TcpFlags,
    pub window_size: u16,
    pub checksum: u16,
    pub urgent_pointer: u16,
    pub options: Vec<u8>,
}

impl TcpHeader {
    pub const MIN_LENGTH: usize = 20;
    pub const MAX_LENGTH: usize = 60;

    pub fn new(source_port: u16, destination_port: u16) -> Self {
        Self {
            source_port,
            destination_port,
            sequence_number: rand::random(),
            acknowledgment_number: 0,
            data_offset: 5, // 20 bytes
            flags: TcpFlags::new(),
            window_size: 65535,
            checksum: 0,
            urgent_pointer: 0,
            options: Vec::new(),
        }
    }

    pub fn parse(packet: &mut Packet) -> NetResult<Self> {
        if packet.len() < Self::MIN_LENGTH {
            return Err(NetError::packet_parse("TCP header too short"));
        }

        let source_port = packet.read_u16()?;
        let destination_port = packet.read_u16()?;
        let sequence_number = packet.read_u32()?;
        let acknowledgment_number = packet.read_u32()?;

        let data_offset_flags = packet.read_u16()?;
        let data_offset = (data_offset_flags >> 12) as u8;
        let flags = TcpFlags::from_u8((data_offset_flags & 0xFF) as u8);

        if data_offset < 5 {
            return Err(NetError::protocol("TCP", "invalid data offset"));
        }

        let window_size = packet.read_u16()?;
        let checksum = packet.read_u16()?;
        let urgent_pointer = packet.read_u16()?;

        // read options if present
        let options_length = (data_offset as usize - 5) * 4;
        let options = if options_length > 0 {
            packet.read_bytes(options_length)?.to_vec()
        } else {
            Vec::new()
        };

        Ok(TcpHeader {
            source_port,
            destination_port,
            sequence_number,
            acknowledgment_number,
            data_offset,
            flags,
            window_size,
            checksum,
            urgent_pointer,
            options,
        })
    }

    pub fn serialize(&self, payload_length: usize) -> Vec<u8> {
        let mut buffer = Vec::with_capacity(self.data_offset as usize * 4);

        buffer.extend_from_slice(&self.source_port.to_be_bytes());
        buffer.extend_from_slice(&self.destination_port.to_be_bytes());
        buffer.extend_from_slice(&self.sequence_number.to_be_bytes());
        buffer.extend_from_slice(&self.acknowledgment_number.to_be_bytes());

        let data_offset_flags = ((self.data_offset as u16) << 12) | (self.flags.to_u8() as u16);
        buffer.extend_from_slice(&data_offset_flags.to_be_bytes());

        buffer.extend_from_slice(&self.window_size.to_be_bytes());
        buffer.extend_from_slice(&[0, 0]); // checksum placeholder
        buffer.extend_from_slice(&self.urgent_pointer.to_be_bytes());
        buffer.extend_from_slice(&self.options);

        buffer
    }

    pub fn calculate_checksum(&mut self, source_ip: &[u8], dest_ip: &[u8], payload: &[u8]) {
        let header = self.serialize(payload.len());
        let mut header_placeholder = Vec::new();

        header_placeholder.extend_from_slice(source_ip);
        header_placeholder.extend_from_slice(dest_ip);
        header_placeholder.push(0); // reserved
        header_placeholder.push(6); // tcp protocol
        header_placeholder.extend_from_slice(&(header.len() + payload.len()).to_be_bytes()[2..]);

        // tcp header + payload
        self.checksum = internet_checksum(&header_placeholder);
    }
}

// tcp segment
#[derive(Debug, Clone)]
pub struct TcpSegment {
    pub header: TcpHeader,
    pub payload: Packet,
}

impl TcpSegment {
    pub fn new(header: TcpHeader, payload: Packet) -> Self {
        Self { header, payload }
    }

    pub fn parse(mut packet: Packet) -> NetResult<Self> {
        let header = TcpHeader::parse(&mut packet)?;
        Ok(TcpSegment {
            header,
            payload: packet,
        })
    }

    pub fn serialize(&mut self, source_ip: &[u8], dest_ip: &[u8]) -> Packet {
        self.header
            .calculate_checksum(source_ip, dest_ip, self.payload.as_slice());
        let header_bytes = self.header.serialize(self.payload.len());

        let mut builder = PacketBuilder::with_capacity(header_bytes.len() + self.payload.len());
        builder.write_bytes(&header_bytes);
        builder.write_bytes(self.payload.as_slice());

        builder.build()
    }
}

// tcp connection tracking
#[derive(Debug, Clone)]
pub struct TcpConnection {
    pub state: TcpState,
    pub local_addr: SocketAddr,
    pub remote_addr: SocketAddr,
    pub local_seq: u32,
    pub remote_seq: u32,
    pub local_ack: u32,
    pub remote_ack: u32,
    pub window_size: u16,
    pub last_activity: Instant,
    pub retransmit_queue: Vec<TcpSegment>,
}

impl TcpConnection {
    pub fn new(local_addr: SocketAddr, remote_addr: SocketAddr) -> Self {
        Self {
            state: TcpState::Closed,
            local_addr,
            remote_addr,
            local_seq: rand::random(),
            remote_seq: 0,
            local_ack: 0,
            remote_ack: 0,
            window_size: 65535,
            last_activity: Instant::now(),
            retransmit_queue: Vec::new(),
        }
    }

    pub fn update_activity(&mut self) {
        self.last_activity = Instant::now();
    }

    pub fn is_expired(&self, timeout: std::time::Duration) -> bool {
        self.last_activity.elapsed() > timeout
    }

    pub fn handle_segment(&mut self, segment: &TcpSegment) -> NetResult<Vec<TcpSegment>> {
        self.update_activity();

        let mut responses = Vec::new();

        match self.state {
            TcpState::Listen => {
                if segment.header.flags.syn {
                    self.remote_seq = segment.header.sequence_number;
                    self.remote_ack = segment.header.sequence_number + 1;
                    self.state = TcpState::SynReceived;

                    // send SYN-ACK
                    let mut response_header =
                        TcpHeader::new(self.local_addr.port(), self.remote_addr.port());

                    response_header.sequence_number = self.local_seq;
                    response_header.acknowledgment_number = self.remote_ack;
                    response_header.flags = TcpFlags::syn_ack();

                    responses.push(TcpSegment::new(
                        response_header,
                        Packet::from_vec(Vec::new()),
                    ));

                    self.local_seq += 1;
                }
            }

            TcpState::SynSent => {
                if segment.header.flags.syn && segment.header.flags.ack {
                    self.remote_seq = segment.header.sequence_number;
                    self.remote_ack = segment.header.sequence_number + 1;
                    self.state = TcpState::Established;

                    // send ACK
                    let mut response_header =
                        TcpHeader::new(self.local_addr.port(), self.remote_addr.port());

                    response_header.sequence_number = self.local_seq;
                    response_header.acknowledgment_number = self.remote_ack;
                    response_header.flags = TcpFlags::ack();

                    responses.push(TcpSegment::new(
                        response_header,
                        Packet::from_vec(Vec::new()),
                    ));
                }
            }

            TcpState::SynReceived => {
                if segment.header.flags.ack {
                    self.state = TcpState::Established;
                }
            }

            TcpState::Established => {
                if segment.header.flags.fin {
                    self.state = TcpState::CloseWait;
                    self.remote_ack = segment.header.sequence_number + 1;

                    // send ACK fo FIN
                    let mut response_header =
                        TcpHeader::new(self.local_addr.port(), self.remote_addr.port());

                    response_header.sequence_number = self.local_seq;
                    response_header.acknowledgment_number = self.remote_ack;
                    response_header.flags = TcpFlags::ack();

                    responses.push(TcpSegment::new(
                        response_header,
                        Packet::from_vec(Vec::new()),
                    ));
                } else if !segment.payload.is_empty() {
                    // data segment
                    self.remote_ack = segment.header.sequence_number + segment.payload.len() as u32;

                    // send ACK
                    let mut response_header =
                        TcpHeader::new(self.local_addr.port(), self.remote_addr.port());

                    response_header.sequence_number = self.local_seq;
                    response_header.acknowledgment_number = self.remote_ack;
                    response_header.flags = TcpFlags::ack();

                    responses.push(TcpSegment::new(
                        response_header,
                        Packet::from_vec(Vec::new()),
                    ));
                }
            }

            _ => {
                // handle other state later :D
            }
        }

        Ok(responses)
    }

    pub fn initiate_connection(&mut self) -> NetResult<TcpSegment> {
        if self.state != TcpState::Closed {
            return Err(NetError::socket("connection already in progress"));
        }

        self.state = TcpState::SynSent;

        let mut header = TcpHeader::new(self.local_addr.port(), self.remote_addr.port());

        header.sequence_number = self.local_seq;
        header.flags = TcpFlags::syn();

        self.local_seq += 1;

        Ok(TcpSegment::new(header, Packet::from_vec(Vec::new())))
    }

    pub fn close_connection(&mut self) -> NetResult<TcpSegment> {
        match self.state {
            TcpState::Established => {
                self.state = TcpState::FinWait1;
            }
            TcpState::CloseWait => {
                self.state = TcpState::LastAck;
            }
            _ => {
                return Err(NetError::socket("invalid state for close"));
            }
        }

        let mut header = TcpHeader::new(self.local_addr.port(), self.remote_addr.port());
        header.sequence_number = self.local_seq;
        header.acknowledgment_number = self.remote_ack;
        header.flags = TcpFlags::fin();

        self.local_seq += 1;

        Ok(TcpSegment::new(header, Packet::from_vec(Vec::new())))
    }
}

// tcp connection manager
pub struct TcpConnectionManager {
    connections: Arc<RwLock<HashMap<(SocketAddr, SocketAddr), TcpConnection>>>,
    connection_timeout: std::time::Duration,
}

impl TcpConnectionManager {
    pub fn new() -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            connection_timeout: std::time::Duration::from_secs(300), // 5 minutes
        }
    }

    pub async fn get_connection(
        &self,
        local: SocketAddr,
        remote: SocketAddr,
    ) -> Option<TcpConnection> {
        let connections = self.connections.read().await;
        connections.get(&(local, remote)).cloned()
    }

    pub async fn add_connection(&self, connection: TcpConnection) {
        let mut connections = self.connections.write().await;
        connections.insert((connection.local_addr, connection.remote_addr), connection);
    }

    pub async fn update_connection(
        &self,
        local: SocketAddr,
        remote: SocketAddr,
        connection: TcpConnection,
    ) {
        let mut connections = self.connections.write().await;
        connections.insert((local, remote), connection);
    }

    pub async fn remove_connection(&self, local: SocketAddr, remote: SocketAddr) {
        let mut connections = self.connections.write().await;
        connections.remove(&(local, remote));
    }

    pub async fn cleanup_expired_connections(&self) {
        let mut connections = self.connections.write().await;
        connections.retain(|_, conn| !conn.is_expired(self.connection_timeout));
    }

    pub async fn handle_segment(
        &self,
        segment: TcpSegment,
        local: SocketAddr,
        remote: SocketAddr,
    ) -> NetResult<Vec<TcpSegment>> {
        let mut connection = self.get_connection(local, remote).await.unwrap_or_else(|| {
            let mut conn = TcpConnection::new(local, remote);
            conn.state = TcpState::Listen;
            conn
        });

        let responses = connection.handle_segment(&segment)?;

        if connection.state == TcpState::Closed {
            self.remove_connection(local, remote).await;
        } else {
            self.update_connection(local, remote, connection).await;
        }

        Ok(responses)
    }
}

impl Default for TcpConnectionManager {
    fn default() -> Self {
        Self::new()
    }
}
