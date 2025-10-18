use crate::error::{NetError, NetResult};
use bytes::{BufMut, Bytes, BytesMut};
use std::net::{Ipv4Addr, Ipv6Addr};

// maximum transmission unit for Ethernet
pub const MTU: usize = 1500;

#[derive(Debug, Clone)]
pub struct Packet {
    data: Bytes,
    offset: usize,
}

impl Packet {
    pub fn new(data: Bytes) -> Packet {
        Packet { data, offset: 0 }
    }

    pub fn from_vec(data: Vec<u8>) -> Self {
        Self::new(Bytes::from(data))
    }

    pub fn len(&self) -> usize {
        self.data.len() - self.offset
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.data[self.offset..]
    }

    // move the reading position forward
    pub fn advance(&mut self, count: usize) -> NetResult<()> {
        if count > self.len() {
            return Err(NetError::BufferOverflow);
        }

        self.offset += count;
        Ok(())
    }

    // look at a byte
    pub fn peek(&self, index: usize) -> NetResult<u8> {
        if index >= self.len() {
            return Err(NetError::BufferOverflow);
        }

        Ok(self.data[self.offset + index])
    }

    // read integer data in network
    pub fn read_u8(&mut self) -> NetResult<u8> {
        if self.len() < 1 {
            return Err(NetError::BufferOverflow);
        }

        let value = self.data[self.offset];
        self.offset += 1;
        Ok(value)
    }

    pub fn read_u16(&mut self) -> NetResult<u16> {
        if self.len() < 2 {
            return Err(NetError::BufferOverflow);
        }

        let value = u16::from_be_bytes([self.data[self.offset], self.data[self.offset + 1]]);
        self.offset += 2;
        Ok(value)
    }

    pub fn read_u32(&mut self) -> NetResult<u32> {
        if self.len() < 4 {
            return Err(NetError::BufferOverflow);
        }

        let value = u32::from_be_bytes([
            self.data[self.offset],
            self.data[self.offset + 1],
            self.data[self.offset + 2],
            self.data[self.offset + 3],
        ]);
        self.offset += 4;
        Ok(value)
    }

    pub fn read_bytes(&mut self, count: usize) -> NetResult<Bytes> {
        if self.len() < count {
            return Err(NetError::BufferOverflow);
        }

        let bytes = self.data.slice(self.offset..self.offset + count);
        self.offset += count;
        Ok(bytes)
    }

    // split packet
    pub fn split_at(&self, index: usize) -> NetResult<(Packet, Packet)> {
        if index > self.len() {
            return Err(NetError::BufferOverflow);
        }

        let left = Packet {
            data: self.data.clone(),
            offset: self.offset,
        };

        let right = Packet {
            data: self.data.clone(),
            offset: self.offset + index,
        };

        Ok((left, right))
    }
}

// packet builder for constructing network packets
pub struct PacketBuilder {
    buffer: BytesMut,
}

impl PacketBuilder {
    pub fn new() -> Self {
        Self {
            buffer: BytesMut::with_capacity(MTU),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            buffer: BytesMut::with_capacity(capacity),
        }
    }

    pub fn write_u8(&mut self, value: u8) {
        self.buffer.put_u8(value);
    }

    pub fn write_u16(&mut self, value: u16) {
        self.buffer.put_u16(value);
    }
    pub fn write_u32(&mut self, value: u32) {
        self.buffer.put_u32(value);
    }
    pub fn write_bytes(&mut self, bytes: &[u8]) {
        self.buffer.put_slice(bytes);
    }

    pub fn write_ipv4(&mut self, addr: Ipv4Addr) {
        self.write_bytes(&addr.octets());
    }
    pub fn write_ipv6(&mut self, addr: Ipv6Addr) {
        self.write_bytes(&addr.octets());
    }
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    pub fn build(self) -> Packet {
        Packet::new(self.buffer.freeze())
    }
}

impl Default for PacketBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// calculate internet checksum (RFC 1071)
pub fn internet_checksum(data: &[u8]) -> u16 {
    let mut sum = 0_u32;
    let mut i = 0;

    // sum all 16bit words
    while i < data.len() - 1 {
        sum += u16::from_be_bytes([data[i], data[i + 1]]) as u32;

        i += 2;
    }

    // add byte if present
    if i < data.len() {
        sum += (data[i] as u32) << 8;
    }

    // add carry bit
    while sum >> 16 != 0 {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }

    !sum as u16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_packet_operations() {
        let data = vec![0x12, 0x34, 0x56, 0x78];
        let mut packet = Packet::from_vec(data);

        assert_eq!(packet.len(), 4);
        assert_eq!(packet.read_u16().unwrap(), 0x1234);
        assert_eq!(packet.read_u16().unwrap(), 0x5678);
        assert_eq!(packet.len(), 0);
    }

    #[test]
    fn test_packet_builder() {
        let mut builder = PacketBuilder::new();
        builder.write_u16(0x1234);
        builder.write_u16(0x5678);

        let packet = builder.build();
        assert_eq!(packet.as_slice(), &[0x12, 0x34, 0x56, 0x78]);
    }

    #[test]
    fn test_internet_checksum() {
        let data = [0x45, 0x00, 0x00, 0x3c, 0x1c, 0x46, 0x40, 0x00];
        let checksum = internet_checksum(&data);

        assert_ne!(checksum, 0); // should produce valid checksum
    }
}
