use crate::error::{NetError, NetResult};
use crate::ethernet::{EthernetFrame, MacAddress};
use std::{collections::HashMap, net::IpAddr, sync::Arc};
use tokio::sync::RwLock;
use tracing::{info, warn};

#[allow(dead_code)]
// network inteface configuration
#[derive(Debug, Clone)]
pub struct InterfaceConfig {
    pub name: String, // eth0, wlan0, etc.
    pub mac_address: MacAddress,
    pub ip_addresses: Vec<IpAddr>,
    pub mtu: usize,
    pub is_up: bool,
    pub is_promiscuous: bool,
}

impl InterfaceConfig {
    pub fn new(name: String, mac_address: MacAddress) -> Self {
        Self {
            name,
            mac_address,
            ip_addresses: Vec::new(),
            mtu: 1500,
            is_up: false,
            is_promiscuous: false,
        }
    }

    pub fn add_ip(&mut self, ip: IpAddr) {
        if !self.ip_addresses.contains(&ip) {
            self.ip_addresses.push(ip);
        }
    }

    pub fn remove_ip(&mut self, ip: &IpAddr) {
        self.ip_addresses.retain(|addr| addr != ip);
    }

    pub fn has_ip(&self, ip: &IpAddr) -> bool {
        self.ip_addresses.contains(ip)
    }
}

// inteface stats
#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct IntefaceStats {
    pub packets_sent: u64,
    pub packets_received: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub errors_sent: u64,
    pub errors_received: u64,
    pub dropped_packets: u64,
}

// network inteface for packet processing
#[allow(dead_code)]
pub struct NetworkInterface {
    config: Arc<RwLock<InterfaceConfig>>,
    stats: Arc<RwLock<IntefaceStats>>,
}

impl NetworkInterface {
    pub fn new(config: InterfaceConfig) -> Self {
        Self {
            config: Arc::new(RwLock::new(config)),
            stats: Arc::new(RwLock::new(IntefaceStats::default())),
        }
    }

    pub async fn bring_up(&mut self) -> NetResult<()> {
        let mut config = self.config.write().await;
        config.is_up = true;
        info!("Network interface {} brought up", config.name);
        Ok(())
    }

    pub async fn bring_down(&mut self) -> NetResult<()> {
        let mut config = self.config.write().await;
        config.is_up = false;
        info!("Network interface {} brought down", config.name);
        Ok(())
    }

    pub async fn send_frame(&self, frame: EthernetFrame) -> NetResult<()> {
        let config = self.config.read().await;

        if !config.is_up {
            return Err(NetError::interface("inteface is down"));
        }

        let frame_size = frame.serialize().len();

        info!(
            "sending frame {frame_size} bytes on interface {}",
            config.name
        );

        let mut stats = self.stats.write().await;
        stats.packets_sent += 1;
        stats.bytes_sent += frame_size as u64;

        Ok(())
    }

    pub async fn receive_frame(&self) -> NetResult<EthernetFrame> {
        let config = self.config.read().await;

        if !config.is_up {
            return Err(NetError::interface("inteface is down"));
        }

        // indicate no frame available
        Err(NetError::interface("no frames available"))
    }

    pub async fn get_config(&self) -> InterfaceConfig {
        self.config.read().await.clone()
    }

    pub async fn update_config<F>(&self, update_fn: F) -> NetResult<()>
    where
        F: FnOnce(&mut InterfaceConfig),
    {
        let mut config = self.config.write().await;

        update_fn(&mut config);
        Ok(())
    }

    pub async fn get_stats(&self) -> IntefaceStats {
        self.stats.read().await.clone()
    }

    pub async fn reset_stats(&self) {
        let mut stats = self.stats.write().await;
        *stats = IntefaceStats::default();
    }

    pub async fn is_up(&self) -> bool {
        self.config.read().await.is_up
    }

    pub async fn get_mac_address(&self) -> MacAddress {
        self.config.read().await.mac_address
    }
    pub async fn get_ip_address(&self) -> Vec<IpAddr> {
        self.config.read().await.ip_addresses.clone()
    }
    pub async fn add_ip_address(&self, ip: IpAddr) -> NetResult<()> {
        let mut config = self.config.write().await;
        config.add_ip(ip);
        Ok(())
    }
    pub async fn remove_ip_address(&self, ip: &IpAddr) -> NetResult<()> {
        let mut config = self.config.write().await;
        config.remove_ip(ip);
        Ok(())
    }
}

// inteface manager
#[allow(dead_code)]
pub struct InterfaceManager {
    interfaces: Arc<RwLock<HashMap<String, Arc<NetworkInterface>>>>,
}

impl InterfaceManager {
    pub fn new() -> Self {
        Self {
            interfaces: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn add_inteface(&self, interface: NetworkInterface) -> NetResult<()> {
        let config = interface.get_config().await;
        let name = config.name.clone();

        let mut interfaces = self.interfaces.write().await;

        if interfaces.contains_key(&name) {
            return Err(NetError::interface(format!(
                "interface {name} already exists"
            )));
        }

        interfaces.insert(name.clone(), Arc::new(interface));

        Ok(())
    }

    pub async fn remove_interface(&self, name: &str) -> NetResult<()> {
        let mut interfaces = self.interfaces.write().await;

        if let Some(interface) = interfaces.remove(name) {
            if interface.is_up().await {
                warn!("interface is still up, bring it down");
            }

            Ok(())
        } else {
            Err(NetError::interface("interface not found"))
        }
    }

    pub async fn get_interface(&self, name: &str) -> Option<Arc<NetworkInterface>> {
        let interfaces = self.interfaces.read().await;
        interfaces.get(name).cloned()
    }

    pub async fn list_interface(&self) -> Vec<String> {
        let interfaces = self.interfaces.read().await;
        interfaces.keys().cloned().collect()
    }

    pub async fn find_interface_by_ip(&self, ip: &IpAddr) -> Option<Arc<NetworkInterface>> {
        let interfaces = self.interfaces.read().await;

        for interface in interfaces.values() {
            let config = interface.get_config().await;
            if config.has_ip(ip) {
                return Some(interface.clone());
            }
        }

        None
    }

    pub async fn find_interface_by_name(&self, mac: &MacAddress) -> Option<Arc<NetworkInterface>> {
        let interfaces = self.interfaces.read().await;

        for interface in interfaces.values() {
            let config = interface.get_config().await;
            if config.mac_address == *mac {
                return Some(interface.clone());
            }
        }

        None
    }

    pub async fn get_default_interface(&self) -> Option<Arc<NetworkInterface>> {
        let interfaces = self.interfaces.read().await;

        // return first interfaces that's up
        for interface in interfaces.values() {
            if interface.is_up().await {
                return Some(interface.clone());
            }
        }

        None
    }
}

impl Default for InterfaceManager {
    fn default() -> Self {
        Self::new()
    }
}
