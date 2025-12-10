# VoidBridge: Custom Layer 3 VPN in Rust

**VoidBridge** is a high-performance, encrypted Layer 3 tunneling protocol built from scratch in Rust. It intercepts network traffic via a TUN interface, encrypts it using ChaCha20-Poly1305, and tunnels it over UDP to a remote peer.

* **Features**
* **Layer 3 Tunneling:** Uses `tun` devices to route IP packets directly.
* **Modern Cryptography:** Authenticated Encryption (AEAD) using **ChaCha20-Poly1305**.
* **Zero-Config Routing:** Automatically configures `iptables` NAT on the server and routing tables on the client.
* **Async Concurrency:** Built on `tokio` for non-blocking I/O.
* **Cross-Platform Logic:** Single binary handles both Client and Server roles.

##Architecture
1.  **Capture:** The kernel routes traffic (e.g., `8.8.8.8`) into the `tun0` interface.
2.  **Processing:** VoidBridge reads the raw IP packet.
3.  **Encryption:**
    * Generates a unique 12-byte **Nonce**.
    * Encrypts payload using a pre-shared 32-byte key.
    * Appends Authentication Tag (Poly1305).
4.  **Transport:** Sends `[Nonce + Ciphertext]` via UDP to the peer.
5.  **Gateway:** The Server decrypts the packet and uses NAT (Masquerading) to forward it to the real internet.

## Installation & Usage

### Prerequisites
* Linux (Kernel 5.x+)
* Rust Toolchain (1.70+)

### Building
```bash
cargo build --release