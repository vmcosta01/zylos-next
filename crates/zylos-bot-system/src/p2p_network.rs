//! P2P Network Transport — QUIC-based federation layer for P2PMessenger
//!
//! Implements real network transport behind the `p2p-network` feature flag.
//! Uses Quinn (QUIC) with rustls for encrypted, multiplexed peer-to-peer
//! communication between Zylos agents across machines.
//!
//! # Architecture
//! - Each node runs a QUIC endpoint (server + client)
//! - Messages are framed: `[4-byte length][blake3 hash (32 bytes)][payload]`
//! - Peer discovery via configurable seed addresses
//! - Integration with existing `P2PMessenger` via `NetworkBridge`

#[cfg(feature = "p2p-network")]
use std::collections::HashMap;
#[cfg(feature = "p2p-network")]
use std::net::SocketAddr;
#[cfg(feature = "p2p-network")]
use std::sync::Arc;
#[cfg(feature = "p2p-network")]
use std::time::Duration;

#[cfg(feature = "p2p-network")]
use blake3;
#[cfg(feature = "p2p-network")]
use quinn::{Connection, Endpoint, Incoming, RecvStream};
#[cfg(feature = "p2p-network")]
use rcgen::{generate_simple_self_signed, CertifiedKey};
#[cfg(feature = "p2p-network")]
use rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer};
#[cfg(feature = "p2p-network")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "p2p-network")]
use tokio::sync::{mpsc, Mutex, RwLock};
#[cfg(feature = "p2p-network")]
use tracing::{debug, info, warn};

#[cfg(feature = "p2p-network")]
use crate::p2p::{Message, Topic};

/// Wire frame header: 4-byte big-endian length + 32-byte blake3 content hash
#[cfg(feature = "p2p-network")]
const HEADER_LEN: usize = 4 + 32;

/// Maximum message size over the wire (16 MB)
#[cfg(feature = "p2p-network")]
const MAX_MESSAGE_SIZE: u32 = 16 * 1024 * 1024;

/// Network-level envelope wrapping a P2P Message for wire transport
#[cfg(feature = "p2p-network")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireEnvelope {
    /// Protocol version for forward compatibility
    pub version: u8,
    /// Node ID of the sender (blake3 hash of node certificate)
    pub node_id: String,
    /// The P2P message payload
    pub message: Message,
}

#[cfg(feature = "p2p-network")]
impl WireEnvelope {
    pub fn new(node_id: impl Into<String>, message: Message) -> Self {
        Self {
            version: 1,
            node_id: node_id.into(),
            message,
        }
    }

    /// Serialize to bytes and compute blake3 integrity hash
    pub fn encode(&self) -> Result<Vec<u8>, NetworkError> {
        let payload =
            serde_json::to_vec(self).map_err(|e| NetworkError::Serialization(e.to_string()))?;

        if payload.len() > MAX_MESSAGE_SIZE as usize {
            return Err(NetworkError::MessageTooLarge(payload.len()));
        }

        let hash = blake3::hash(&payload);
        let len_bytes = (payload.len() as u32).to_be_bytes();

        let mut frame = Vec::with_capacity(HEADER_LEN + payload.len());
        frame.extend_from_slice(&len_bytes);
        frame.extend_from_slice(hash.as_bytes());
        frame.extend_from_slice(&payload);

        Ok(frame)
    }

    /// Decode from raw frame bytes (after header has been read)
    pub fn decode(header: &[u8; HEADER_LEN], payload: &[u8]) -> Result<Self, NetworkError> {
        // Verify blake3 integrity
        let expected_hash = &header[4..36];
        let actual_hash = blake3::hash(payload);
        if actual_hash.as_bytes() != expected_hash {
            return Err(NetworkError::IntegrityMismatch);
        }

        serde_json::from_slice(payload).map_err(|e| NetworkError::Deserialization(e.to_string()))
    }
}

/// Errors specific to the P2P network transport layer
#[cfg(feature = "p2p-network")]
#[derive(Debug, thiserror::Error)]
pub enum NetworkError {
    #[error("QUIC endpoint error: {0}")]
    Endpoint(String),

    #[error("Connection failed: {0}")]
    Connection(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Deserialization error: {0}")]
    Deserialization(String),

    #[error("Message too large: {0} bytes (max {MAX_MESSAGE_SIZE})")]
    MessageTooLarge(usize),

    #[error("Integrity check failed: blake3 hash mismatch")]
    IntegrityMismatch,

    #[error("Peer not found: {0}")]
    PeerNotFound(String),

    #[error("Transport shutdown")]
    Shutdown,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Configuration for the P2P network transport
#[cfg(feature = "p2p-network")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// Local bind address for the QUIC endpoint
    pub bind_addr: SocketAddr,
    /// Seed peer addresses for initial discovery
    pub seeds: Vec<SocketAddr>,
    /// Human-readable node identifier
    pub node_id: String,
    /// Connection timeout in milliseconds
    pub connect_timeout_ms: u64,
    /// Keep-alive interval in seconds
    pub keep_alive_secs: u64,
}

#[cfg(feature = "p2p-network")]
impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            bind_addr: "0.0.0.0:9700".parse().unwrap(),
            seeds: Vec::new(),
            node_id: format!("zylos-node-{}", uuid::Uuid::new_v4()),
            connect_timeout_ms: 5000,
            keep_alive_secs: 30,
        }
    }
}

/// Manages QUIC connections to remote peers and bridges to local P2PMessenger
#[cfg(feature = "p2p-network")]
pub struct NetworkTransport {
    config: NetworkConfig,
    endpoint: Option<Endpoint>,
    peers: Arc<RwLock<HashMap<String, Connection>>>,
    /// Channel for incoming messages from remote peers → local messenger
    inbound_tx: mpsc::Sender<Message>,
    inbound_rx: Mutex<Option<mpsc::Receiver<Message>>>,
    shutdown_tx: Option<tokio::sync::watch::Sender<bool>>,
}

#[cfg(feature = "p2p-network")]
impl NetworkTransport {
    /// Create a new network transport (does not start listening yet)
    pub fn new(config: NetworkConfig) -> Self {
        let (inbound_tx, inbound_rx) = mpsc::channel(1024);
        Self {
            config,
            endpoint: None,
            peers: Arc::new(RwLock::new(HashMap::new())),
            inbound_tx,
            inbound_rx: Mutex::new(Some(inbound_rx)),
            shutdown_tx: None,
        }
    }

    /// Generate self-signed TLS certificate for QUIC
    fn generate_tls_config(
        &self,
    ) -> Result<(quinn::ServerConfig, quinn::ClientConfig), NetworkError> {
        let subject_alt_names = vec!["localhost".to_string(), self.config.node_id.clone()];
        let CertifiedKey { cert, key_pair } = generate_simple_self_signed(subject_alt_names)
            .map_err(|e| NetworkError::Endpoint(format!("cert generation: {}", e)))?;

        let cert_der = CertificateDer::from(cert.der().to_vec());
        let key_der = PrivatePkcs8KeyDer::from(key_pair.serialize_der());

        // Server config
        let server_crypto = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(vec![cert_der.clone()], key_der.clone_key().into())
            .map_err(|e| NetworkError::Endpoint(format!("server TLS: {}", e)))?;

        let mut server_config = quinn::ServerConfig::with_crypto(Arc::new(
            quinn::crypto::rustls::QuicServerConfig::try_from(server_crypto)
                .map_err(|e| NetworkError::Endpoint(format!("QUIC server: {}", e)))?,
        ));
        server_config.transport_config(Arc::new({
            let mut tc = quinn::TransportConfig::default();
            tc.keep_alive_interval(Some(Duration::from_secs(self.config.keep_alive_secs)));
            tc
        }));

        // Client config (accept any server cert for now — production uses pinned certs)
        let mut roots = rustls::RootCertStore::empty();
        roots.add(cert_der).ok();
        let client_crypto = rustls::ClientConfig::builder()
            .with_root_certificates(roots)
            .with_no_client_auth();

        let client_config = quinn::ClientConfig::new(Arc::new(
            quinn::crypto::rustls::QuicClientConfig::try_from(client_crypto)
                .map_err(|e| NetworkError::Endpoint(format!("QUIC client: {}", e)))?,
        ));

        Ok((server_config, client_config))
    }

    /// Start the QUIC endpoint: bind, accept incoming connections, connect to seeds
    pub async fn start(&mut self) -> Result<(), NetworkError> {
        let (server_config, client_config) = self.generate_tls_config()?;

        let endpoint = Endpoint::server(server_config, self.config.bind_addr)
            .map_err(|e| NetworkError::Endpoint(e.to_string()))?;

        info!(addr = %self.config.bind_addr, node = %self.config.node_id, "P2P QUIC endpoint started");

        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        self.shutdown_tx = Some(shutdown_tx);

        // Spawn accept loop
        let peers = self.peers.clone();
        let inbound_tx = self.inbound_tx.clone();
        let mut shutdown_accept = shutdown_rx.clone();
        let accept_endpoint = endpoint.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = shutdown_accept.changed() => {
                        info!("P2P accept loop shutting down");
                        break;
                    }
                    incoming = accept_endpoint.accept() => {
                        match incoming {
                            Some(conn) => {
                                let peers = peers.clone();
                                let tx = inbound_tx.clone();
                                tokio::spawn(Self::handle_incoming(conn, peers, tx));
                            }
                            None => break,
                        }
                    }
                }
            }
        });

        // Connect to seed peers
        for seed in &self.config.seeds {
            let ep = endpoint.clone();
            let cc = client_config.clone();
            let peers = self.peers.clone();
            let tx = self.inbound_tx.clone();
            let addr = *seed;
            let node_id = self.config.node_id.clone();
            tokio::spawn(async move {
                match ep.connect_with(cc, addr, &node_id) {
                    Ok(connecting) => match connecting.await {
                        Ok(conn) => {
                            info!(peer = %addr, "Connected to seed peer");
                            peers.write().await.insert(addr.to_string(), conn.clone());
                            Self::read_loop(conn, tx).await;
                        }
                        Err(e) => {
                            warn!(peer = %addr, error = %e, "Seed handshake failed");
                        }
                    },
                    Err(e) => {
                        warn!(peer = %addr, error = %e, "Failed to initiate seed connection");
                    }
                }
            });
        }

        self.endpoint = Some(endpoint);
        Ok(())
    }

    /// Handle an incoming QUIC connection
    async fn handle_incoming(
        incoming: Incoming,
        peers: Arc<RwLock<HashMap<String, Connection>>>,
        inbound_tx: mpsc::Sender<Message>,
    ) {
        match incoming.await {
            Ok(conn) => {
                let remote = conn.remote_address().to_string();
                info!(peer = %remote, "Accepted incoming QUIC connection");
                peers.write().await.insert(remote.clone(), conn.clone());
                Self::read_loop(conn, inbound_tx).await;
                peers.write().await.remove(&remote);
            }
            Err(e) => {
                warn!(error = %e, "Incoming connection handshake failed");
            }
        }
    }

    /// Read frames from a QUIC connection and forward decoded messages to inbound channel
    async fn read_loop(conn: Connection, inbound_tx: mpsc::Sender<Message>) {
        loop {
            match conn.accept_bi().await {
                Ok((_send, recv)) => {
                    let tx = inbound_tx.clone();
                    tokio::spawn(Self::read_stream(recv, tx));
                }
                Err(quinn::ConnectionError::ApplicationClosed(_)) => {
                    debug!("Connection closed by peer");
                    break;
                }
                Err(e) => {
                    warn!(error = %e, "Connection error, stopping read loop");
                    break;
                }
            }
        }
    }

    /// Read a single framed message from a receive stream
    async fn read_stream(mut recv: RecvStream, inbound_tx: mpsc::Sender<Message>) {
        // Read header
        let mut header_buf = [0u8; HEADER_LEN];
        if recv.read_exact(&mut header_buf).await.is_err() {
            return; // Stream closed
        }

        // Parse length
        let len = u32::from_be_bytes([header_buf[0], header_buf[1], header_buf[2], header_buf[3]])
            as usize;

        if len > MAX_MESSAGE_SIZE as usize {
            warn!(size = len, "Rejecting oversized message");
            return;
        }

        // Read payload
        let mut payload = vec![0u8; len];
        if recv.read_exact(&mut payload).await.is_err() {
            warn!("Stream closed mid-payload");
            return;
        }

        // Decode and verify integrity
        match WireEnvelope::decode(&header_buf, &payload) {
            Ok(envelope) => {
                debug!(
                    from_node = %envelope.node_id,
                    topic = %envelope.message.topic,
                    msg_id = %envelope.message.id,
                    "Received remote message"
                );
                if inbound_tx.send(envelope.message).await.is_err() {
                    warn!("Inbound channel closed, dropping message");
                }
            }
            Err(e) => {
                warn!(error = %e, "Failed to decode wire envelope");
            }
        }
    }

    /// Send a message to a specific peer by address
    pub async fn send_to_peer(
        &self,
        peer_addr: &str,
        message: Message,
    ) -> Result<(), NetworkError> {
        let peers = self.peers.read().await;
        let conn = peers
            .get(peer_addr)
            .ok_or_else(|| NetworkError::PeerNotFound(peer_addr.to_string()))?;

        let envelope = WireEnvelope::new(&self.config.node_id, message);
        let frame = envelope.encode()?;

        let mut send = conn
            .open_bi()
            .await
            .map_err(|e| NetworkError::Connection(e.to_string()))?
            .0;

        send.write_all(&frame)
            .await
            .map_err(|e| NetworkError::Connection(e.to_string()))?;

        send.finish()
            .map_err(|e| NetworkError::Connection(e.to_string()))?;

        debug!(peer = %peer_addr, "Message sent to peer");
        Ok(())
    }

    /// Broadcast a message to all connected peers
    pub async fn broadcast(&self, message: Message) -> Result<usize, NetworkError> {
        let peers = self.peers.read().await;
        let envelope = WireEnvelope::new(&self.config.node_id, message);
        let frame = envelope.encode()?;

        let mut sent_count = 0;
        for (addr, conn) in peers.iter() {
            match conn.open_bi().await {
                Ok((mut send, _recv)) => {
                    if send.write_all(&frame).await.is_ok() && send.finish().is_ok() {
                        sent_count += 1;
                    } else {
                        warn!(peer = %addr, "Failed to send broadcast frame");
                    }
                }
                Err(e) => {
                    warn!(peer = %addr, error = %e, "Failed to open stream for broadcast");
                }
            }
        }

        debug!(peers = peers.len(), sent = sent_count, "Broadcast complete");
        Ok(sent_count)
    }

    /// Take the inbound message receiver (can only be called once)
    pub async fn take_inbound_receiver(&self) -> Option<mpsc::Receiver<Message>> {
        self.inbound_rx.lock().await.take()
    }

    /// Get list of currently connected peer addresses
    pub async fn connected_peers(&self) -> Vec<String> {
        self.peers.read().await.keys().cloned().collect()
    }

    /// Gracefully shut down the transport
    pub async fn shutdown(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(true);
        }
        if let Some(ep) = self.endpoint.take() {
            ep.close(0u32.into(), b"shutdown");
        }
        self.peers.write().await.clear();
        info!("P2P network transport shut down");
    }
}

/// Bridge between `NetworkTransport` (remote QUIC) and `P2PMessenger` (local broadcast).
///
/// Routes messages bidirectionally:
/// - Local publishes → forwarded to all connected remote peers
/// - Remote inbound → injected into local messenger topics
#[cfg(feature = "p2p-network")]
pub struct NetworkBridge {
    transport: Arc<NetworkTransport>,
    messenger: Arc<crate::p2p::P2PMessenger>,
}

#[cfg(feature = "p2p-network")]
impl NetworkBridge {
    pub fn new(transport: Arc<NetworkTransport>, messenger: Arc<crate::p2p::P2PMessenger>) -> Self {
        Self {
            transport,
            messenger,
        }
    }

    /// Start bridging: spawns two async tasks for bidirectional routing.
    /// Returns a shutdown handle.
    pub async fn start(&self) -> Result<tokio::sync::watch::Sender<bool>, NetworkError> {
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

        // Task 1: Forward remote inbound messages into local P2PMessenger
        if let Some(mut inbound_rx) = self.transport.take_inbound_receiver().await {
            let messenger = self.messenger.clone();
            let mut shutdown_inbound = shutdown_rx.clone();
            tokio::spawn(async move {
                loop {
                    tokio::select! {
                        _ = shutdown_inbound.changed() => break,
                        msg = inbound_rx.recv() => {
                            match msg {
                                Some(message) => {
                                    debug!(
                                        from = %message.sender,
                                        topic = %message.topic,
                                        "Bridging remote message to local messenger"
                                    );
                                    if let Err(e) = messenger.publish(message).await {
                                        warn!(error = %e, "Failed to bridge remote message locally");
                                    }
                                }
                                None => break, // Channel closed
                            }
                        }
                    }
                }
                info!("Inbound bridge task stopped");
            });
        } else {
            warn!("No inbound receiver available — remote→local bridge disabled");
        }

        // Task 2: Forward local P2PMessenger publishes to all remote peers
        // Subscribe to a wildcard-like "federation" topic that local agents use
        // when they want messages forwarded to the network.
        let transport_outbound = self.transport.clone();
        let messenger_outbound = self.messenger.clone();
        let mut shutdown_outbound = shutdown_rx.clone();
        tokio::spawn(async move {
            // Subscribe to the special federation relay topic
            let mut rx = messenger_outbound.subscribe("__federation_relay__");
            loop {
                tokio::select! {
                    _ = shutdown_outbound.changed() => break,
                    result = rx.recv() => {
                        match result {
                            Ok(message) => {
                                debug!(
                                    from = %message.sender,
                                    topic = %message.topic,
                                    "Forwarding local message to remote peers"
                                );
                                if let Err(e) = transport_outbound.broadcast(message).await {
                                    warn!(error = %e, "Failed to forward local message to remote peers");
                                }
                            }
                            Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                                warn!(skipped = n, "Federation relay lagged, skipping messages");
                            }
                            Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                        }
                    }
                }
            }
            info!("Outbound bridge task stopped");
        });

        Ok(shutdown_tx)
    }

    /// Publish a message both locally AND to all remote peers.
    pub async fn publish_federated(&self, msg: Message) -> Result<(usize, usize), NetworkError> {
        // Local delivery first
        let local_count = self
            .messenger
            .publish(msg.clone())
            .await
            .map_err(|e| NetworkError::Connection(format!("local publish: {}", e)))?;

        // Remote broadcast
        let remote_count = self.transport.broadcast(msg).await?;

        Ok((local_count, remote_count))
    }

    /// Send a message to a specific remote peer AND publish locally.
    pub async fn send_federated(
        &self,
        peer_addr: &str,
        msg: Message,
    ) -> Result<usize, NetworkError> {
        // Local delivery
        self.messenger
            .publish(msg.clone())
            .await
            .map_err(|e| NetworkError::Connection(format!("local publish: {}", e)))?;

        // Remote targeted send
        self.transport.send_to_peer(peer_addr, msg).await?;

        Ok(1)
    }

    /// Publish a message to the federation relay topic.
    ///
    /// Local agents call this when they want a message forwarded to all
    /// connected remote peers via the outbound bridge task spawned by
    /// [`start`](Self::start). The message is delivered locally first (so
    /// local subscribers on `__federation_relay__` also see it), then the
    /// outbound bridge picks it up and broadcasts it over the QUIC transport.
    pub async fn relay_to_peers(&self, msg: Message) -> Result<usize, NetworkError> {
        let mut relay_msg = msg;
        relay_msg.topic = crate::p2p::Topic::new("__federation_relay__");
        self.messenger
            .publish(relay_msg)
            .await
            .map_err(|e| NetworkError::Connection(format!("relay publish: {}", e)))
    }

    /// Get current federation status.
    pub async fn status(&self) -> FederationStatus {
        FederationStatus {
            connected_peers: self.transport.connected_peers().await,
            local_topics: self.messenger.list_topics(),
        }
    }
}

/// Snapshot of the current federation state.
#[cfg(feature = "p2p-network")]
#[derive(Debug, Clone)]
pub struct FederationStatus {
    pub connected_peers: Vec<String>,
    pub local_topics: Vec<(Topic, usize)>,
}

#[cfg(all(test, feature = "p2p-network"))]
mod tests {
    use super::*;

    #[test]
    fn test_wire_envelope_roundtrip() {
        let msg = Message::new("test-topic", "node-a", serde_json::json!({"key": "value"}));
        let envelope = WireEnvelope::new("test-node", msg);

        let encoded = envelope.encode().expect("encode should succeed");

        // Extract header and payload
        let mut header = [0u8; HEADER_LEN];
        header.copy_from_slice(&encoded[..HEADER_LEN]);
        let payload = &encoded[HEADER_LEN..];

        let decoded = WireEnvelope::decode(&header, payload).expect("decode should succeed");
        assert_eq!(decoded.node_id, "test-node");
        assert_eq!(decoded.version, 1);
        assert_eq!(decoded.message.sender, "node-a");
        assert_eq!(decoded.message.payload["key"], "value");
    }

    #[test]
    fn test_wire_envelope_integrity_tamper() {
        let msg = Message::new("topic", "sender", serde_json::json!({}));
        let envelope = WireEnvelope::new("node", msg);
        let encoded = envelope.encode().unwrap();

        let mut header = [0u8; HEADER_LEN];
        header.copy_from_slice(&encoded[..HEADER_LEN]);
        let mut payload = encoded[HEADER_LEN..].to_vec();

        // Tamper with payload
        if !payload.is_empty() {
            payload[0] ^= 0xFF;
        }

        let result = WireEnvelope::decode(&header, &payload);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            NetworkError::IntegrityMismatch
        ));
    }

    #[test]
    fn test_wire_envelope_oversized() {
        let huge = vec![0u8; MAX_MESSAGE_SIZE as usize + 1];
        let msg = Message::new(
            "big",
            "sender",
            serde_json::Value::String(String::from_utf8(huge).unwrap()),
        );
        let envelope = WireEnvelope::new("node", msg);
        let result = envelope.encode();
        assert!(matches!(result, Err(NetworkError::MessageTooLarge(_))));
    }

    #[test]
    fn test_network_config_default() {
        let cfg = NetworkConfig::default();
        assert_eq!(cfg.bind_addr.port(), 9700);
        assert!(cfg.seeds.is_empty());
        assert!(cfg.node_id.starts_with("zylos-node-"));
    }
}
