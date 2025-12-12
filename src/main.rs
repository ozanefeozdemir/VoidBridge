use chacha20poly1305::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    ChaCha20Poly1305, Nonce
};
use clap::{Parser, Subcommand};
use std::error::Error;
use std::net::SocketAddr;
use std::process::Command; // NEW: To run shell commands
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UdpSocket;

const MTU: usize = 1500;
const KEY_BYTES: [u8; 32] = [
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 
    16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31
];

#[derive(Parser)]
#[command(name = "VoidBridge")]
struct Cli {
    #[command(subcommand)]
    mode: Mode,
}

#[derive(Subcommand, PartialEq)]
enum Mode {
    Server {
        #[arg(long, default_value_t = 9000)]
        port: u16,
        /// The physical interface to masquerade traffic through (e.g., enp0s3)
        #[arg(long)]
        nat_interface: Option<String>,
    },
    Client {
        #[arg(long)]
        remote_ip: String,
        #[arg(long, default_value_t = 9000)]
        port: u16,
    },
}

/// Helper to run shell commands easily
fn run_cmd(cmd: &str, args: &[&str]) {
    let output = Command::new(cmd)
        .args(args)
        .output()
        .expect("Failed to execute command");
    
    if !output.status.success() {
        eprintln!("Command failed: {} {:?}", cmd, args);
        eprintln!("Stderr: {}", String::from_utf8_lossy(&output.stderr));
    } else {
        println!("Executed: {} {:?}", cmd, args);
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();
    
    let cipher = ChaCha20Poly1305::new(&KEY_BYTES.into());
    let cipher = Arc::new(cipher); 

    let (tun_ip, socket, remote_addr, is_server, nat_iface) = match &cli.mode {
        Mode::Server { port, nat_interface } => {
            let socket = UdpSocket::bind(format!("0.0.0.0:{}", port)).await?;
            println!("VoidBridge SERVER (Encrypted) listening on 0.0.0.0:{}", port);
            ("10.0.0.1", socket, None, true, nat_interface.clone())
        }
        Mode::Client { remote_ip, port } => {
            let socket = UdpSocket::bind("0.0.0.0:0").await?;
            let remote_string = format!("{}:{}", remote_ip, port);
            let remote_addr: SocketAddr = remote_string.parse()?;
            socket.connect(remote_addr).await?;
            println!("VoidBridge CLIENT (Encrypted) connected to {}", remote_string);
            ("10.0.0.2", socket, Some(remote_addr), false, None)
        }
    };

    // --- SETUP TUN INTERFACE ---
    let mut config = tun::Configuration::default();
    config
        .address(tun_ip.parse::<std::net::IpAddr>()?)
        .netmask((255, 255, 255, 0))
        .up();

    #[cfg(target_os = "linux")]
    config.platform(|config| {
        config.packet_information(true);
    });

    let dev = tun::create_as_async(&config)?;
    let (mut tun_reader, mut tun_writer) = tokio::io::split(dev);

    println!("Interface {} is UP.", tun_ip);

    // --- AUTOMATION: FIREWALL & ROUTING ---
    if is_server {
        // SERVER AUTOMATION
        if let Some(iface) = nat_iface {
            println!("Configuring NAT on interface: {}", iface);
            // 1. Enable IP Forwarding
            run_cmd("sysctl", &["-w", "net.ipv4.ip_forward=1"]);
            // 2. Add Iptables Rules
            run_cmd("iptables", &["-t", "nat", "-A", "POSTROUTING", "-o", &iface, "-j", "MASQUERADE"]);
            run_cmd("iptables", &["-A", "FORWARD", "-i", "tun0", "-j", "ACCEPT"]);
            run_cmd("iptables", &["-A", "FORWARD", "-m", "state", "--state", "RELATED,ESTABLISHED", "-j", "ACCEPT"]);
        }
    } else {
        // CLIENT AUTOMATION
        println!("Redirecting Global Traffic into Tunnel...");
        // 1. Route 0.0.0.0/1 (First half of internet)
        run_cmd("ip", &["route", "add", "0.0.0.0/1", "dev", "tun0"]);
        // 2. Route 128.0.0.0/1 (Second half of internet)
        run_cmd("ip", &["route", "add", "128.0.0.0/1", "dev", "tun0"]);
    }

    let peer_addr = Arc::new(tokio::sync::Mutex::new(remote_addr));
    let socket = Arc::new(socket);

    println!("Tunnel Ready. Traffic is flowing.");

    // --- MAIN LOOP (Copy-Paste from previous step) ---
    let mut buf_tun = [0u8; MTU];
    let mut buf_udp = [0u8; MTU + 50]; 

    loop {
        let peer_addr_read = peer_addr.clone();
        let peer_addr_write = peer_addr.clone();
        let cipher_send = cipher.clone();
        let cipher_recv = cipher.clone();
        
        tokio::select! {
            res = tun_reader.read(&mut buf_tun) => {
                let n = res?;
                if n > 0 {
                    let target = *peer_addr_read.lock().await;
                    if let Some(addr) = target {
                        let nonce = ChaCha20Poly1305::generate_nonce(&mut OsRng);
                        match cipher_send.encrypt(&nonce, &buf_tun[..n]) {
                            Ok(encrypted) => {
                                let mut packet = nonce.to_vec();
                                packet.extend_from_slice(&encrypted);
                                socket.send_to(&packet, addr).await?;
                            }
                            Err(_) => eprintln!("Encryption Failed!"),
                        }
                    }
                }
            }
            res = socket.recv_from(&mut buf_udp) => {
                let (n, src) = res?;
                if n > 0 {
                    let mut lock = peer_addr_write.lock().await;
                    if lock.is_none() { println!("[+] New Peer: {}", src); }
                    *lock = Some(src);
                    if n < 12 { continue; }
                    let nonce = Nonce::from_slice(&buf_udp[0..12]);
                    let ciphertext = &buf_udp[12..n];
                    match cipher_recv.decrypt(nonce, ciphertext) {
                        Ok(plaintext) => { tun_writer.write_all(&plaintext).await?; },
                        Err(_) => {}
                    }
                }
            }
        }
    }
}