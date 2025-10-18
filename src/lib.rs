pub mod error;
pub mod ethernet;
pub mod interface;
pub mod ip;
pub mod packet;
pub mod socket;
pub mod stack;
pub mod tcp;
pub mod utils;

pub use error::{NetError, NetResult};
pub use interface::NetworkInterface;
pub use socket::{TcpSocket, UdpSocket};
pub use stack::NetworkStack;

// init networking stack
// with default config
pub async fn init() -> NetResult<NetworkStack> {
    NetworkStack::new().await
}
