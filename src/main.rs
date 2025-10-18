use tcpstack::{
    NetworkInterface, NetworkStack, ethernet::MacAddress, interface::InterfaceConfig, utils,
};

use clap::{Parser, Subcommand};
use std::{
    net::{IpAddr, SocketAddr},
    os::linux::raw::stat,
    time::Duration,
};

use tokio::time::sleep;
use tracing::{Level, error, info};

#[derive(Parser)]
#[command(name = "rustnet-demo")]
#[command(about = "A demo application for the RUST TCP/IP Stack")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[arg(short, long, default_value = "info")]
    log_level: String,
}

#[derive(Subcommand)]
enum Commands {
    /// start a TCP server
    TcpServer {
        #[arg(short, long, default_value = "127.0.0.1:8080")]
        bind: String,
    },
    /// connect as TCP client
    TcpClient {
        #[arg(short, long)]
        connect: String,

        #[arg(short, long, default_value = "Hello, TCP!")]
        message: String,
    },
    /// start a UDP server
    UdpServer {
        #[arg(short, long, default_value = "127.0.0.1:8080")]
        bind: String,
    },
    /// send UDP message
    UdpClient {
        #[arg(short, long)]
        target: String,

        #[arg(short, long, default_value = "Hello, T!")]
        message: String,
    },
    /// show stack stats
    Stats {
        #[arg(short, long)]
        watch: bool,
    },
    /// network interface management
    Interface {
        #[command(subcommand)]
        action: InterfaceAction,
    },
}

#[derive(Subcommand)]
enum InterfaceAction {
    /// list all intefaces
    List,
    /// add a new interface
    Add {
        name: String,
        #[arg(short, long)]
        mac: Option<String>,
        #[arg(short, long)]
        ip: Option<String>,
    },
    /// remove an interface
    Remove { name: String },
    /// show interface details
    Show { name: String },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // init logging
    let log_level = match cli.log_level.as_str() {
        "trace" => Level::TRACE,
        "debug" => Level::DEBUG,
        "warn" => Level::WARN,
        "error" => Level::ERROR,
        _ => Level::INFO,
    };

    tracing_subscriber::fmt()
        .with_max_level(log_level)
        .with_target(false)
        .init();

    // init the network stack
    let stack = NetworkStack::new().await?;
    info!("network stack initialized");

    // add a defautl loopback interface
    let loopback_mac = MacAddress::new([0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
    let mut loopback_config = InterfaceConfig::new("lo".to_string(), loopback_mac);
    loopback_config.add_ip("127.0.0.1".parse()?);
    loopback_config.add_ip("::1".parse()?);
    let loopback_interface = NetworkInterface::new(loopback_config);
    stack.add_interface(loopback_interface).await?;

    match cli.command {
        Commands::TcpServer { bind } => {
            run_tcp_server(&stack, &bind).await?;
        }
        Commands::TcpClient { connect, message } => {
            run_tcp_client(&stack, &connect, &message).await?;
        }
        Commands::UdpServer { bind } => {
            run_udp_server(&stack, &bind).await?;
        }
        Commands::UdpClient { target, message } => {
            run_udp_client(&stack, &target, &message).await?;
        }
        Commands::Stats { watch } => {
            show_stats(&stack, watch).await?;
        }
        Commands::Interface { action } => {
            handle_interface_command(&stack, action).await?;
        }
    }

    Ok(())
}

async fn run_tcp_server(
    stack: &NetworkStack,
    bind_addr: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("starting TCP server on {bind_addr}");

    let addr: SocketAddr = bind_addr.parse()?;
    let mut socket = stack.create_tcp_socket();

    socket.bind(addr).await?;
    socket.listen(10).await?;

    // keep the server running
    loop {
        sleep(Duration::from_secs(1)).await;
        let stats = stack.get_stats().await;
        if stats.packets_processed > 0 {
            info!("processed {} packets", stats.packets_processed);
        }
    }
}

async fn run_tcp_client(
    stack: &NetworkStack,
    connect_addr: &str,
    message: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let addr: SocketAddr = connect_addr.parse()?;
    let mut socket = stack.create_tcp_socket();

    // simulate connection timeout
    match socket.connect(addr).await {
        Ok(_) => {
            info!("Connected successfully");
            let _ = socket.send(message.as_bytes()).await;
            info!("message sent");
        }
        Err(e) => {
            error!("connection failed {e}");
        }
    }
    Ok(())
}

async fn run_udp_server(
    stack: &NetworkStack,
    bind_addr: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let addr: SocketAddr = bind_addr.parse()?;
    let mut socket = stack.create_udp_socket();

    socket.bind(addr).await?;

    // keep the server running
    loop {
        sleep(Duration::from_secs(1)).await;
        let stats = stack.get_stats().await;
        if stats.udp_datagrams_received > 0 {
            info!("received {} udp datagrams", stats.udp_datagrams_received);
        }
    }
}

async fn run_udp_client(
    stack: &NetworkStack,
    target_addr: &str,
    message: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let addr: SocketAddr = target_addr.parse()?;
    let mut socket = stack.create_udp_socket();

    let local_addr = SocketAddr::new("127.0.0.1".parse()?, 0);
    socket.bind(local_addr).await?;

    match socket.send_to(message.as_bytes(), addr).await {
        Ok(bytes_sent) => {
            info!("sent {bytes_sent} bytes");
        }
        Err(e) => {
            error!("send failed {e}");
        }
    }
    Ok(())
}

async fn show_stats(stack: &NetworkStack, watch: bool) -> Result<(), Box<dyn std::error::Error>> {
    if watch {
        info!("watching stack stats (Ctrl+C to exit)");
        loop {
            print_stats(stack).await;
            sleep(Duration::from_secs(1)).await;
        }
    } else {
        print_stats(stack).await;
    }

    Ok(())
}

async fn print_stats(stack: &NetworkStack) {
    let stats = stack.get_stats().await;

    println!("\n=== Network Stack Statistic ===");
    println!("Packets processed:         {}", stats.packets_processed);
    println!("Packets dropped:           {}", stats.packets_dropped);
    println!("Packets forwarded:         {}", stats.packets_forwarded);
    println!(
        "TCP connections active:    {}",
        stats.tcp_connections_active
    );
    println!("TCP connections total:     {}", stats.tcp_connections_total);
    println!("UDP datagrams sent:        {}", stats.udp_datagrams_sent);
    println!(
        "UDP datagrams received:    {}",
        stats.udp_datagrams_received
    );
    println!("ARP requests sent:        {}", stats.arp_requests_sent);
    println!("ARP responses received:   {}", stats.arp_requests_received);
    println!("ICMP messages sent:       {}", stats.icmp_messages_sent);
    println!("ICMP messages received:   {}", stats.icmp_messages_received);
    println!("total errors:       {}", stats.errors_total);
}

async fn handle_interface_command(
    stack: &NetworkStack,
    action: InterfaceAction,
) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        InterfaceAction::List => {
            let interfaces = stack.interface_manager.list_interface().await;

            for name in interfaces {
                if let Some(interface) = stack.get_interface(&name).await {
                    let config = interface.get_config().await;
                    let stats = interface.get_stats().await;
                    println!(
                        "    {} - {} ({})",
                        name,
                        config.mac_address,
                        if config.is_up { "UP" } else { "DOWN" }
                    );

                    for ip in &config.ip_addresses {
                        println!("    IP: {ip}");
                    }

                    println!(
                        "    RX: {} packets, {}",
                        stats.packets_received,
                        utils::format_bytes(stats.bytes_received)
                    );
                    println!(
                        "    TX: {} packets, {}",
                        stats.packets_sent,
                        utils::format_bytes(stats.bytes_sent)
                    );
                }
            }
        }

        InterfaceAction::Add { name, mac, ip } => {
            let mac_addr = if let Some(mac_str) = mac {
                mac_str.parse()?
            } else {
                utils::generate_random_mac()
            };

            let mut config = InterfaceConfig::new(name.clone(), mac_addr);

            if let Some(ip_str) = ip {
                let ip_addr: IpAddr = ip_str.parse()?;
                config.add_ip(ip_addr);
            }

            let interface = NetworkInterface::new(config);
            stack.add_interface(interface).await?;

            info!("added interface: {name}");
        }

        InterfaceAction::Remove { name } => {
            stack.remove_interface(&name).await?;
            info!("remove interface: {name}");
        }

        InterfaceAction::Show { name } => {
            if let Some(interface) = stack.get_interface(&name).await {
                let config = interface.get_config().await;
                let stats = interface.get_stats().await;

                println!("interface: {}", config.name);
                println!("  MAC address: {}", config.mac_address);
                println!("  Status: {}", if config.is_up { "UP" } else { "DOWN" });
                println!("  MTU: {}", config.mtu);
                println!("  IP addresses:");
                for ip in &config.ip_addresses {
                    println!("   {ip}");
                }
                println!("  Statistics:");
                println!(
                    "   RX: {} packets, {}",
                    stats.packets_received,
                    utils::format_bytes(stats.bytes_received)
                );
                println!(
                    "   TX: {} packets, {}",
                    stats.packets_sent,
                    utils::format_bytes(stats.bytes_sent)
                );
                println!(
                    "   Errors: RX={}, TX={}",
                    stats.errors_received, stats.errors_sent,
                );
                println!("   Dropped: {}", stats.dropped_packets);
            } else {
                error!("interface {name} not found");
            }
        }
    }

    Ok(())
}
