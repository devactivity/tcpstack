use crate::{
    error::NetResult,
    interface::{InterfaceManager, NetworkInterface},
    socket::{TcpSocket, UdpSocket},
    tcp::TcpConnectionManager,
};

use std::{sync::Arc, time::Duration};
use tokio::{
    sync::{RwLock, mpsc},
    task::JoinHandle,
};
use tracing::info;

// network stack configuration
#[derive(Debug, Clone)]
pub struct StackConfig {
    pub enable_ipv4: bool,
    pub enable_ipv6: bool,
    pub enable_icmp: bool,
    pub tcp_connection_timeout: Duration,
    pub tcp_retransmit_timeout: Duration,
    pub tcp_max_retransmits: u32,
    pub arp_timeout: Duration,
    pub packet_buffer_size: usize,
    pub worker_threads: usize,
}

impl Default for StackConfig {
    fn default() -> Self {
        Self {
            enable_ipv4: true,
            enable_ipv6: true,
            enable_icmp: true,
            tcp_connection_timeout: Duration::from_secs(300),
            tcp_retransmit_timeout: Duration::from_millis(200),
            tcp_max_retransmits: 3,
            arp_timeout: Duration::from_secs(300),
            packet_buffer_size: 1_000,
            worker_threads: 4,
        }
    }
}

// network stack stats
#[derive(Debug, Default, Clone)]
pub struct StackStats {
    pub packets_processed: u64,
    pub packets_dropped: u64,
    pub packets_forwarded: u64,
    pub tcp_connections_active: u64,
    pub tcp_connections_total: u64,
    pub udp_datagrams_sent: u64,
    pub udp_datagrams_received: u64,
    pub arp_requests_sent: u64,
    pub arp_requests_received: u64,
    pub icmp_messages_sent: u64,
    pub icmp_messages_received: u64,
    pub errors_total: u64,
}

// main network stack
pub struct NetworkStack {
    config: StackConfig,
    pub interface_manager: Arc<InterfaceManager>,
    tcp_connection_manager: Arc<TcpConnectionManager>,
    stats: Arc<RwLock<StackStats>>,

    // worker handles
    worker_handles: Vec<JoinHandle<()>>,

    // shutdown signal
    shutdown_tx: mpsc::UnboundedSender<()>,
}

impl NetworkStack {
    // create a new network stack
    pub async fn new() -> NetResult<Self> {
        Self::with_config(StackConfig::default()).await
    }

    pub async fn with_config(config: StackConfig) -> NetResult<Self> {
        let interface_manager = Arc::new(InterfaceManager::new());
        let tcp_connection_manager = Arc::new(TcpConnectionManager::new());

        let (shutdown_tx, shutdown_rx) = mpsc::unbounded_channel();

        let mut stack = Self {
            config,
            interface_manager,
            tcp_connection_manager,
            stats: Arc::new(RwLock::new(StackStats::default())),
            worker_handles: Vec::new(),
            shutdown_tx,
        };

        // start worker tasks
        stack.start_workers().await?;

        Ok(stack)
    }

    async fn start_workers(&mut self) -> NetResult<()> {
        // placeholder worker
        let stats = self.stats.clone();
        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            loop {
                interval.tick().await;
                let stats = stats.read().await;

                info!(
                    "
                        Stack stats: {} packets processed, {} errors
                    ",
                    stats.packets_processed, stats.errors_total
                );
            }
        });

        self.worker_handles.push(handle);
        Ok(())
    }

    // add a network interface
    pub async fn add_interface(&self, interface: NetworkInterface) -> NetResult<()> {
        self.interface_manager.add_inteface(interface).await
    }
    // remove a network interface
    pub async fn remove_interface(&self, name: &str) -> NetResult<()> {
        self.interface_manager.remove_interface(name).await
    }

    // get a network interface
    pub async fn get_interface(&self, name: &str) -> Option<Arc<NetworkInterface>> {
        self.interface_manager.get_interface(name).await
    }

    // create a TCP socket
    pub fn create_tcp_socket(&self) -> TcpSocket {
        TcpSocket::new(self.tcp_connection_manager.clone())
    }

    pub fn create_udp_socket(&self) -> UdpSocket {
        UdpSocket::new()
    }

    // get stack stats
    pub async fn get_stats(&self) -> StackStats {
        self.stats.read().await.clone()
    }

    // reset stact stats
    pub async fn reset_stats(&self) {
        let mut stats = self.stats.write().await;
        *stats = StackStats::default();
    }

    // shutdown the network stack
    pub async fn shutdown(&mut self) -> NetResult<()> {
        // send shutdown signal
        let _ = self.shutdown_tx.send(());

        // wait for worker tasks to complete
        for handle in self.worker_handles.drain(..) {
            handle.abort();
        }

        info!("network stack shutdown complete");
        Ok(())
    }
}
