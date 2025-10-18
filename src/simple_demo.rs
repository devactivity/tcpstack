use tcpstack::error::{NetError, NetResult};
use tcpstack::{
    ip::{IpProtocol, Ipv4Header},
    packet::{Packet, PacketBuilder},
    tcp::{TcpFlags, TcpHeader, TcpState},
};

use clap::{Parser, Subcommand};
use std::{
    collections::HashMap,
    net::{Ipv4Addr, SocketAddr},
    sync::Arc,
    time::Duration,
};
use tokio::{
    sync::{Mutex, RwLock},
    time::{sleep, timeout},
};

use tracing::{error, info, warn};

#[derive(Parser)]
#[command(name = "rust-tcp-demo")]
#[command(about = "a TCP demo using my own stack")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    #[arg(short, long, default_value = "info")]
    log_level: String,
}

#[derive(Subcommand)]
enum Commands {
    /// start a simple TCP server
    Server {
        #[arg(short, long, default_value = "8080")]
        port: u16,
    },
    /// connect as TCP client
    Client {
        #[arg(short = 'H', long, default_value = "127.0.0.1")]
        host: String,
        #[arg(short, long, default_value = "8080")]
        port: u16,
        #[arg(short, long, default_value = "Hello from RustTCP")]
        message: String,
    },
    /// demonstrate packet creation and parsing
    // PacketDemo,
    // /// Run TCP/IP stack demo
    // RealStack,
    /// start hybrid server (stack + network)
    Hybrid {
        #[arg(short, long, default_value = "9999")]
        port: u16,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // init log
    let log_level = match cli.log_level.as_str() {
        "trace" => tracing::Level::TRACE,
        "debug" => tracing::Level::DEBUG,
        "warn" => tracing::Level::WARN,
        "error" => tracing::Level::ERROR,
        _ => tracing::Level::INFO,
    };

    tracing_subscriber::fmt()
        .with_max_level(log_level)
        .with_target(false)
        .init();

    match cli.command {
        Commands::Server { port } => {
            run_real_tcp_server(port).await?;
        }
        Commands::Client {
            host,
            port,
            message,
        } => {
            run_real_tcp_client(&host, port, &message).await?;
        }
        // Commands::PacketDemo => {
        //     demonstrate_packet_creation().await?;
        // }
        // Commands::RealStack => {
        //     run_real_working_demo().await?;
        // }
        Commands::Hybrid { port } => {
            run_hybrid_demo(port).await?;
        }
    }

    Ok(())
}

async fn run_real_tcp_server(port: u16) -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting REAL TCP server on port  {port}");
    info!("this uses standard TCP sockets for comparison with the stack");

    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{port}")).await?;

    info!("server listening on port  {port}");
    info!(
        "
connect with: cargo run --bin simple-tcp-demo client --port {port}
"
    );
    info!("Or use: nc 127.0.0.1 {port}");

    loop {
        match listener.accept().await {
            Ok((socket, addr)) => {
                info!("new connection from: {addr}");

                tokio::spawn(async move {
                    let mut buffer = [0_u8; 1024];

                    loop {
                        match socket.try_read(&mut buffer) {
                            Ok(0) => {
                                info!("connection closed by {addr}");
                                break;
                            }
                            Ok(n) => {
                                let message = String::from_utf8_lossy(&buffer[..n]);
                                info!("received from {addr}: {}", message.trim());

                                // echo the message back
                                let response = format!("Echo: {}\n", message.trim());
                                if let Err(e) = socket.try_write(response.as_bytes()) {
                                    error!("failed to send response: {e}");
                                    break;
                                }
                                info!("echoed response back to {addr}");
                            }
                            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                                // no data available
                                sleep(Duration::from_millis(10)).await;
                                continue;
                            }
                            Err(e) => {
                                error!("failed to read from socket: {e}");
                                break;
                            }
                        }
                    }
                });
            }
            Err(e) => {
                error!("failed to accept connection: {e}");
            }
        }
    }
}

async fn run_real_tcp_client(
    host: &str,
    port: u16,
    message: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("connection to real TCP server at {host}:{port}");

    let addr = format!("{host}:{port}");

    match timeout(
        Duration::from_secs(5),
        tokio::net::TcpStream::connect(&addr),
    )
    .await
    {
        Ok(Ok(stream)) => {
            info!("connected successfully to {addr}");

            // send message
            if let Err(e) = stream.try_write(message.as_bytes()) {
                error!("failed to send message: {e}");
                return Ok(());
            }

            info!("sent: {message}");

            // wait for response
            let mut buffer = [0_u8; 1024];
            let mut attempts = 0;

            loop {
                match stream.try_read(&mut buffer) {
                    Ok(n) if n > 0 => {
                        let response = String::from_utf8_lossy(&buffer[..n]);
                        info!("received: {}", response.trim());
                        break;
                    }
                    Ok(0) => {
                        warn!("connection closed by server");
                        break;
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        attempts += 1;
                        if attempts > 100 {
                            warn!("timeout waiting for response");
                            break;
                        }

                        sleep(Duration::from_millis(50)).await;
                        continue;
                    }
                    Err(e) => {
                        error!("failed to read response: {e}");
                        break;
                    }
                    _ => {
                        error!("failed to response");
                        break;
                    }
                }
            }
        }
        Ok(Err(e)) => {
            info!("failed to connect to {addr}: {e}");
            info!(
                "make sure the server is running:
                cargo run --bin simple-tcp-demo server --port {port}"
            );
        }
        Err(_) => {
            error!("connection timeout to {addr}");
            info!(
                "make sure the server is running:
                cargo run --bin simple-tcp-demo server --port {port}"
            );
        }
    }

    Ok(())
}

async fn demonstrate_packet_creation() -> Result<(), Box<dyn std::error::Error>> {
    info!("=== Demonstrating packet creation and parsing ===");

    // create a TCP SYN packet
    let src_ip: Ipv4Addr = "192.168.1.100".parse()?;
    let dst_ip: Ipv4Addr = "192.168.1.1".parse()?;

    // create TCP header
    let mut tcp_header = TcpHeader::new(12345, 80);
    tcp_header.flags = TcpFlags::syn();
    tcp_header.sequence_number = 1000;
    tcp_header.window_size = 65535;

    info!("Created TCP SYN packet:");
    info!("   source port: {}", tcp_header.source_port);
    info!("   dest port: {}", tcp_header.destination_port);
    info!("   sequence: {}", tcp_header.sequence_number);
    info!("   flags: SYN={}", tcp_header.flags.syn);

    // create IP header
    let mut ip_header = Ipv4Header::new(src_ip, dst_ip, IpProtocol::Tcp);
    ip_header.identification = 0x1234;
    ip_header.time_to_live = 64;

    info!("Created IPv4 header:");
    info!("   source IP: {}", ip_header.source);
    info!("   dest ip: {}", ip_header.destination);
    info!("   TTL: {}", ip_header.time_to_live);
    info!("   protocol: TCP");

    info!("packet created successfully");

    info!("=== demonstrating packet parsing ===");

    // create a simple parsing packet
    let mut packet_builder = PacketBuilder::new();

    // add IPv4 header
    packet_builder.write_u8(0x45); // version + IHL
    packet_builder.write_u8(0x00); // ToS
    packet_builder.write_u16(0x00c); // total length
    packet_builder.write_u16(0x1234); // ID
    packet_builder.write_u16(0x4000); // flags + fragment
    packet_builder.write_u8(64); // TTL
    packet_builder.write_u8(6); // protocol TCP
    packet_builder.write_u16(0x0000); // checksum (placeholder)
    packet_builder.write_bytes(&src_ip.octets());
    packet_builder.write_bytes(&dst_ip.octets());

    // add tcp header
    packet_builder.write_u16(12345); // source port
    packet_builder.write_u16(80); // dst port
    packet_builder.write_u32(1000); // sequence
    packet_builder.write_u32(0); // ack
    packet_builder.write_u16(0x5002); // header len + SYN flag
    packet_builder.write_u16(65535); // window
    packet_builder.write_u16(0x0000); // checksum
    packet_builder.write_u16(0x0000); // urgent pointer 

    // add payload
    packet_builder.write_bytes(b"hello, TCP");

    let sample_packet = packet_builder.build();

    // parse the packet
    match parse_sample_packet(sample_packet).await {
        Ok(()) => {
            info!("packet parsing demonstration completed successfully");
        }
        Err(e) => {
            error!("packet parsing failed: {e}");
        }
    }

    Ok(())
}

async fn parse_sample_packet(mut packet: Packet) -> Result<(), Box<dyn std::error::Error>> {
    info!("parsing sample packet ({} bytes)", packet.len());

    // parse ip header
    let ip_header = Ipv4Header::parse(&mut packet)?;
    info!("parsed IP header:");
    info!("  version: {}", ip_header.version);
    info!("  source: {}", ip_header.source);
    info!("  dest: {}", ip_header.destination);
    info!("  protocol: {:?}", ip_header.protocol);
    info!("  TTL: {}", ip_header.time_to_live);

    // parse TCP header
    if ip_header.protocol == IpProtocol::Tcp {
        let tcp_header = TcpHeader::parse(&mut packet)?;
        info!("parse TCP header:");
        info!("  source port: {}", tcp_header.source_port);
        info!("  dest port: {}", tcp_header.destination_port);
        info!("  sequence: {}", tcp_header.sequence_number);
        info!("  SYN: {}", tcp_header.flags.syn);
        info!("  window: {}", tcp_header.window_size);

        // parse payload
        if !packet.is_empty() {
            let payload_data = packet.as_slice();
            let payload_str = String::from_utf8_lossy(payload_data);
            info!("payload: '{payload_str}'");
        }
    }

    Ok(())
}

async fn run_hybrid_demo(port: u16) -> Result<(), Box<dyn std::error::Error>> {
    info!("=== hybrid demo: stack ===");
    info!("");

    let stack = RealNetworkStack::new();

    info!("starting TCP server on port {port}");
    info!("");
    info!("try these connections:");
    info!("   nc 127.0.0.1 {port}");
    info!("cargo run --bin simple-tcp-demo client --port {port}");

    info!("");
    info!("type messages and watch our TCP/IP stack in action");
    info!("press ctrl+c to stop");
    info!("");

    // start the server - this will run 4ever until Ctrl+C
    stack.start_server(port).await?;
    Ok(())
}

pub struct RealNetworkStack {
    connections: Arc<RwLock<HashMap<u16, Arc<Mutex<RealTcpConnection>>>>>,
}

pub struct RealTcpConnection {
    pub local_port: u16,
    pub remote_addr: SocketAddr,
    pub state: TcpState,
    pub local_seq: u32,
    pub remote_seq: u32,
    pub socket: Option<tokio::net::TcpStream>,
    pub last_activity: std::time::Instant,
}

impl Default for RealNetworkStack {
    fn default() -> Self {
        Self::new()
    }
}

impl RealNetworkStack {
    pub fn new() -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn start_server(&self, port: u16) -> NetResult<()> {
        info!("starting TCP server with stack on port {port}");

        let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{port}"))
            .await
            .map_err(|e| NetError::socket(format!("failed to binmd: {e}")))?;

        info!("server listening on 127.0.0.1:{port}");
        info!("connect with: nc 127.0.0.1 {port}");
        info!("");

        loop {
            match listener.accept().await {
                Ok((socket, addr)) => {
                    info!("=== new connection from {addr} ===");

                    self.demonstrate_connection_establishment(&addr, port)
                        .await?;

                    let stack_clone = self.clone();
                    tokio::spawn(async move {
                        if let Err(e) = stack_clone.handle_client_connection(socket, addr).await {
                            error!("error handling client {addr}: {e}");
                        }
                    });
                }
                Err(e) => {
                    error!("failed to accecpt connection : {e}");
                }
            }
        }
    }

    async fn demonstrate_connection_establishment(
        &self,
        client_addr: &SocketAddr,
        server_port: u16,
    ) -> NetResult<()> {
        info!("=== our TCP/IP: connection establishment ===");

        info!("TCP 3-way handshake processing:");
        info!("  1. client -> server: SYN");
        info!("  2. server -> client: SYN-ACK");
        info!("  3. client -> server: ACK");

        let mut tcp_header = TcpHeader::new(server_port, client_addr.port());
        tcp_header.sequence_number = rand::random();
        tcp_header.acknowledgment_number = rand::random::<u32>() + 1;
        tcp_header.flags = TcpFlags::syn_ack();
        tcp_header.window_size = 65535;

        info!("SYN-ACK packet created:");
        info!("  local: 127.0.0.1:{server_port}");
        info!("  remote: {client_addr}");
        info!("  SEQ: {}", tcp_header.sequence_number);
        info!("  ACK: {}", tcp_header.acknowledgment_number);
        info!("  Flags: SYN-ACK");
        info!("  Window: {}", tcp_header.window_size);
        info!("");

        Ok(())
    }
    async fn handle_client_connection(
        &self,
        socket: tokio::net::TcpStream,
        client_addr: SocketAddr,
    ) -> NetResult<()> {
        info!("handling client connection from {client_addr}");

        let mut buffer = [0_u8; 1024];
        let mut message_count = 0;
        loop {
            match socket.try_read(&mut buffer) {
                Ok(0) => {
                    info!("client {client_addr} disconnected gracefully");
                    self.demonstrate_connection_close(&client_addr).await?;
                    break;
                }
                Ok(n) => {
                    message_count += 1;
                    let message = String::from_utf8_lossy(&buffer[..n]);

                    info!(
                        "message #{message_count} from {client_addr}: '{}'",
                        message.trim()
                    );

                    self.demonstrate_data_processing(&message, n).await?;

                    let response = format!("TCP echo #{message_count}: {}\n", message.trim());

                    match socket.try_write(response.as_bytes()) {
                        Ok(sent) => {
                            self.demonstrate_response_creation(&response).await?;
                        }
                        Err(e) => {
                            error!("failed to send response : {e}");
                            break;
                        }
                    }

                    info!("");
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    sleep(Duration::from_millis(10)).await;
                    continue;
                }

                Err(e) => {
                    info!("client {client_addr} disconnected: {e}");
                    self.demonstrate_connection_close(&client_addr).await?;
                    break;
                }
            }
        }

        Ok(())
    }

    async fn demonstrate_data_processing(&self, message: &str, bytes: usize) -> NetResult<()> {
        info!("=== data processing ===");
        info!("processing {bytes} bytes: {}", message.trim());

        Ok(())
    }

    async fn demonstrate_response_creation(&self, response: &str) -> NetResult<()> {
        info!("=== response creation ===");

        let mut tcp_header = TcpHeader::new(9999, 12345);
        tcp_header.sequence_number = rand::random();
        tcp_header.acknowledgment_number = rand::random();
        tcp_header.flags = TcpFlags::ack();
        tcp_header.flags.psh = true; // flag for immedate delivery

        info!("response packet created:");
        info!("payload '{}' ({} bytes)", response.trim(), response.len());

        info!("SEQ: {}", tcp_header.sequence_number);
        info!("ACK: {}", tcp_header.acknowledgment_number);
        info!("Flags: ACK+PSH (data delivery)");

        Ok(())
    }

    async fn demonstrate_connection_close(&self, client_addr: &SocketAddr) -> NetResult<()> {
        info!("=== connection closing ===");

        let mut tcp_header = TcpHeader::new(9999, client_addr.port());
        tcp_header.sequence_number = rand::random();
        tcp_header.acknowledgment_number = rand::random();
        tcp_header.flags = TcpFlags::fin();
        tcp_header.flags.ack = true; // FIN-ACK

        info!("FIN packet stack would send:");
        info!("SEQ: {}", tcp_header.sequence_number);
        info!("Flags: ACK+ACK");
        info!("");

        Ok(())
    }
}

impl Clone for RealNetworkStack {
    fn clone(&self) -> Self {
        Self {
            connections: self.connections.clone(),
        }
    }
}
