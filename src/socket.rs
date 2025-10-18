use crate::error::{NetError, NetResult};
use crate::tcp::{TcpConnection, TcpConnectionManager};
use std::sync::Arc;
use std::{
    collections::VecDeque,
    net::{IpAddr, SocketAddr},
    time::Duration,
};
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, info};

// socket types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketType {
    Tcp,
    Udp,
}

// socket data
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketState {
    Closed,
    Bound,
    Listening,
    Connected,
    Connecting,
    Disconnecting,
}

// tcp socket implementation
pub struct TcpSocket {
    local_addr: Option<SocketAddr>,
    remote_addr: Option<SocketAddr>,
    state: Arc<RwLock<SocketState>>,
    connection_manager: Arc<TcpConnectionManager>,
    incoming_data: Arc<Mutex<VecDeque<Vec<u8>>>>,
    receive_buffer_size: usize,
    send_buffer_size: usize,
    connect_timeout: Duration,
}

impl TcpSocket {
    pub fn new(connection_manager: Arc<TcpConnectionManager>) -> Self {
        Self {
            local_addr: None,
            remote_addr: None,
            state: Arc::new(RwLock::new(SocketState::Closed)),
            connection_manager,
            incoming_data: Arc::new(Mutex::new(VecDeque::new())),
            receive_buffer_size: 65536,
            send_buffer_size: 65536,
            connect_timeout: Duration::from_secs(30),
        }
    }

    pub async fn bind(&mut self, addr: SocketAddr) -> NetResult<()> {
        let mut state = self.state.write().await;

        if *state != SocketState::Closed {
            return Err(NetError::socket(
                "
              socket is already bound or connected
            ",
            ));
        }

        self.local_addr = Some(addr);
        *state = SocketState::Bound;

        info!("TCP socket bound to {addr}");
        Ok(())
    }

    pub async fn listen(&mut self, _backlog: u32) -> NetResult<()> {
        let mut state = self.state.write().await;

        if *state != SocketState::Bound {
            return Err(NetError::socket(
                "
              socket must be bound before listening
            ",
            ));
        }

        *state = SocketState::Listening;

        // info!("TCP socket listening on :{}", self.local_addr);
        Ok(())
    }

    pub async fn accept(&self) -> NetResult<(TcpSocket, SocketAddr)> {
        let mut state = self.state.read().await;

        if *state != SocketState::Listening {
            return Err(NetError::socket(
                "
              socket is not listening
            ",
            ));
        }

        // for demonstration only
        Err(NetError::socket("Accept not yet implemented"))
    }

    pub async fn connect(&mut self, addr: SocketAddr) -> NetResult<()> {
        let mut state = self.state.write().await;

        if *state != SocketState::Closed && *state != SocketState::Bound {
            return Err(NetError::socket(
                "
              socket is already connected or connecting
            ",
            ));
        }

        *state = SocketState::Connecting;
        drop(state);

        self.remote_addr = Some(addr);

        if self.local_addr.is_none() {
            // auto bind to any available port
            let local_ip = match addr.ip() {
                IpAddr::V4(_) => IpAddr::V4("0.0.0.0".parse().unwrap()),
                IpAddr::V6(_) => IpAddr::V6("::".parse().unwrap()),
            };
            self.local_addr = Some(SocketAddr::new(local_ip, 0));
        }

        let local_addr = self.local_addr.unwrap();
        let mut connection = TcpConnection::new(local_addr, addr);

        // initiate connection
        let init_connection = connection.initiate_connection()?;

        // simulate a successful connection
        let mut state = self.state.write().await;
        *state = SocketState::Connected;

        Ok(())
    }

    pub async fn send(&mut self, data: &[u8]) -> NetResult<usize> {
        let mut state = self.state.read().await;

        if *state != SocketState::Connected {
            return Err(NetError::socket(
                "
              socketis not connected
            ",
            ));
        }

        if data.len() > self.send_buffer_size {
            return Err(NetError::socket(
                "
              data is too large for send buffer
            ",
            ));
        }

        let local_addr = self
            .local_addr
            .ok_or_else(|| NetError::socket("no local address"))?;
        let remote_addr = self
            .remote_addr
            .ok_or_else(|| NetError::socket("no remote address"))?;

        Ok(data.len())
    }

    pub async fn receive(&self, buffer: &mut [u8]) -> NetResult<usize> {
        let mut state = self.state.read().await;

        if *state != SocketState::Connected {
            return Err(NetError::socket(
                "
              socket is not connected
            ",
            ));
        }
        drop(state);

        let mut incoming = self.incoming_data.lock().await;

        if let Some(data) = incoming.pop_front() {
            let bytes_to_copy = std::cmp::min(buffer.len(), data.len());
            buffer[..bytes_to_copy].copy_from_slice(&data[..bytes_to_copy]);

            // if we could not copy all data, put the remainder back
            if bytes_to_copy < data.len() {
                incoming.push_front(data[bytes_to_copy..].to_vec());
            }

            Ok(bytes_to_copy)
        } else {
            // no data available
            Ok(0)
        }
    }

    pub async fn close(&mut self) -> NetResult<()> {
        let mut state = self.state.write().await;

        if *state != SocketState::Closed {
            return Ok(());
        }
        *state == SocketState::Disconnecting;
        drop(state);

        if let (Some(local_addr), Some(remote_addr)) = (self.local_addr, self.remote_addr) {
            if let Some(mut connection) = self
                .connection_manager
                .get_connection(local_addr, remote_addr)
                .await
            {
                // send FIN
                let _fin_segment = connection.close_connection()?;
            }
        }

        let mut state = self.state.write().await;
        *state = SocketState::Closed;

        Ok(())
    }

    pub async fn get_state(&self) -> SocketState {
        *self.state.read().await
    }

    pub fn local_addr(&self) -> Option<SocketAddr> {
        self.local_addr
    }
    pub fn remote_addr(&self) -> Option<SocketAddr> {
        self.remote_addr
    }

    pub async fn set_receive_buffer_size(&mut self, size: usize) {
        self.receive_buffer_size = size
    }
    pub async fn set_send_buffer_size(&mut self, size: usize) {
        self.send_buffer_size = size
    }

    pub async fn set_connect_timeout(&mut self, timeout: Duration) {
        self.connect_timeout = timeout;
    }
}

// UDP socket implmentation
pub struct UdpSocket {
    local_addr: Option<SocketAddr>,
    state: Arc<RwLock<SocketState>>,
    incoming_datagram: Arc<Mutex<VecDeque<(Vec<u8>, SocketAddr)>>>,
    receive_buffer_size: usize,
    send_buffer_size: usize,
}

impl UdpSocket {
    pub fn new() -> Self {
        Self {
            local_addr: None,
            state: Arc::new(RwLock::new(SocketState::Closed)),
            incoming_datagram: Arc::new(Mutex::new(VecDeque::new())),
            receive_buffer_size: 65536,
            send_buffer_size: 65536,
        }
    }

    pub async fn bind(&mut self, addr: SocketAddr) -> NetResult<()> {
        let mut state = self.state.write().await;

        if *state != SocketState::Closed {
            return Err(NetError::socket("socket is already bound"));
        }

        self.local_addr = Some(addr);
        *state = SocketState::Bound;

        Ok(())
    }

    pub async fn send_to(&self, data: &[u8], addr: SocketAddr) -> NetResult<usize> {
        let mut state = self.state.read().await;

        if *state != SocketState::Bound {
            return Err(NetError::socket("socket is not bound"));
        }

        if data.len() > self.send_buffer_size {
            return Err(NetError::socket("data too large for send buffer"));
        }

        let local_addr = self
            .local_addr
            .ok_or_else(|| NetError::socket("no local address"))?;

        debug!(
            "sending {} bytes from {} to {}",
            data.len(),
            local_addr,
            addr
        );
        Ok(data.len())
    }

    pub async fn receive_from(&self, buffer: &mut [u8]) -> NetResult<(usize, SocketAddr)> {
        let mut state = self.state.read().await;

        if *state != SocketState::Bound {
            return Err(NetError::socket("socket is not bound"));
        }
        drop(state);

        let mut incoming = self.incoming_datagram.lock().await;

        if let Some((data, sender_addr)) = incoming.pop_front() {
            let bytes_to_copy = std::cmp::min(buffer.len(), data.iter().len());
            buffer[..bytes_to_copy].copy_from_slice(&data[..bytes_to_copy]);

            Ok((bytes_to_copy, sender_addr))
        } else {
            // no data available
            Err(NetError::socket("no data available"))
        }
    }

    pub async fn close(&mut self) -> NetResult<()> {
        let mut state = self.state.write().await;

        *state = SocketState::Closed;

        Ok(())
    }

    pub async fn get_state(&self) -> SocketState {
        *self.state.read().await
    }

    pub fn local_addr(&self) -> Option<SocketAddr> {
        self.local_addr
    }

    pub async fn set_receive_buffer_size(&mut self, size: usize) {
        self.receive_buffer_size = size;
    }
    pub async fn set_send_buffer_size(&mut self, size: usize) {
        self.send_buffer_size = size;
    }
}

impl Default for UdpSocket {
    fn default() -> Self {
        Self::new()
    }
}
