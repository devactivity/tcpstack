use thiserror::Error;

pub type NetResult<T> = Result<T, NetError>;

#[derive(Error, Debug)]
pub enum NetError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Network interface error: {message}")]
    Interface { message: String },

    #[error("Protocol error: {protocol} - {message}")]
    Protocol { protocol: String, message: String },

    #[error("Packet parsing error: {0}")]
    PacketParse(String),

    #[error("Socket error: {0}")]
    Socket(String),

    #[error("Checksum validation failed")]
    ChecksumMismatch,

    #[error("Connection timeout")]
    Timeout,

    #[error("Connection refused")]
    ConnectionRefused,

    #[error("Invalid address: {0}")]
    InvalidAddress(String),

    #[error("Buffer overflow")]
    BufferOverflow,

    #[error("Configuration error: {0}")]
    Configuration(String),
}

impl NetError {
    pub fn interface(msg: impl Into<String>) -> Self {
        NetError::Interface {
            message: msg.into(),
        }
    }

    pub fn protocol(proto: impl Into<String>, msg: impl Into<String>) -> Self {
        NetError::Protocol {
            protocol: proto.into(),
            message: msg.into(),
        }
    }

    pub fn packet_parse(msg: impl Into<String>) -> Self {
        NetError::PacketParse(msg.into())
    }

    pub fn socket(msg: impl Into<String>) -> Self {
        NetError::Socket(msg.into())
    }
}
