use std::net::{SocketAddr, TcpListener};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

use crate::error::{Result, RtspError};
use crate::media::Packetizer;
use crate::media::h264::H264Packetizer;
use crate::mount::{DEFAULT_MOUNT_PATH, MountRegistry};
use crate::session::SessionManager;
use crate::transport::UdpTransport;
use crate::transport::tcp;

/// Server-level configuration used by protocol handlers.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Public host advertised in SDP `o=` and `c=` lines.
    /// When `None`, host is inferred from request URI/client address.
    pub public_host: Option<String>,
    /// Public RTSP port for future URL-based headers (e.g. RTP-Info).
    pub public_port: Option<u16>,
    /// SDP origin username field (`o=<username> ...`).
    pub sdp_username: String,
    /// SDP origin session id field (`o=... <session-id> ...`).
    pub sdp_session_id: String,
    /// SDP origin session version field (`o=... ... <session-version> ...`).
    pub sdp_session_version: String,
    /// SDP session name (`s=`).
    pub sdp_session_name: String,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            public_host: None,
            public_port: None,
            sdp_username: "-".to_string(),
            sdp_session_id: "0".to_string(),
            sdp_session_version: "0".to_string(),
            sdp_session_name: "Stream".to_string(),
        }
    }
}

/// High-level RTSP server orchestrator.
///
/// Owns the mount registry, session manager, and transport layer.
/// Delegates TCP connection handling to [`crate::transport::tcp`] and
/// RTP delivery to [`UdpTransport`].
///
/// # Simple usage (single stream)
///
/// ```no_run
/// use rtsp::Server;
/// let mut server = Server::new("0.0.0.0:8554");
/// server.start().unwrap();
/// // server.send_frame(&h264_data, 3000).unwrap();
/// ```
///
/// # Multi-mount usage
///
/// ```no_run
/// use rtsp::Server;
/// use rtsp::media::h264::H264Packetizer;
/// let mut server = Server::new("0.0.0.0:8554");
/// server.add_mount("/cam1", Box::new(H264Packetizer::with_random_ssrc(96)));
/// server.start().unwrap();
/// // server.send_frame_to("/cam1", &data, 3000).unwrap();
/// ```
pub struct Server {
    session_manager: SessionManager,
    mounts: MountRegistry,
    running: Arc<AtomicBool>,
    bind_addr: String,
    udp: Option<UdpTransport>,
    config: Arc<ServerConfig>,
}

impl Server {
    /// Create a server with a default H.264 mount at `/stream`.
    ///
    /// `bind_addr` must be `host:port` with an explicit non-zero port (e.g. `127.0.0.1:8554`).
    /// Port 0 is not allowed; validation happens in [`start`](Self::start).
    pub fn new(bind_addr: &str) -> Self {
        Self::with_config(bind_addr, ServerConfig::default())
    }

    /// Create a server with custom protocol/SDP configuration.
    /// A default H.264 mount at `/stream` is created automatically.
    pub fn with_config(bind_addr: &str, config: ServerConfig) -> Self {
        let mounts = MountRegistry::new();
        mounts.add(
            DEFAULT_MOUNT_PATH,
            Box::new(H264Packetizer::with_random_ssrc(96)),
        );
        mounts.set_default(DEFAULT_MOUNT_PATH);

        Self {
            session_manager: SessionManager::new(),
            mounts,
            running: Arc::new(AtomicBool::new(false)),
            bind_addr: bind_addr.to_string(),
            udp: None,
            config: Arc::new(config),
        }
    }

    /// Create a server with a custom packetizer on the default mount.
    pub fn with_packetizer(bind_addr: &str, packetizer: Box<dyn Packetizer>) -> Self {
        Self::with_packetizer_and_config(bind_addr, packetizer, ServerConfig::default())
    }

    /// Create a server with a custom packetizer and protocol/SDP configuration.
    pub fn with_packetizer_and_config(
        bind_addr: &str,
        packetizer: Box<dyn Packetizer>,
        config: ServerConfig,
    ) -> Self {
        let mounts = MountRegistry::new();
        mounts.add(DEFAULT_MOUNT_PATH, packetizer);
        mounts.set_default(DEFAULT_MOUNT_PATH);

        Self {
            session_manager: SessionManager::new(),
            mounts,
            running: Arc::new(AtomicBool::new(false)),
            bind_addr: bind_addr.to_string(),
            udp: None,
            config: Arc::new(config),
        }
    }

    /// Register a named mount with its own packetizer.
    ///
    /// Must be called before [`start`](Self::start).
    pub fn add_mount(&self, path: &str, packetizer: Box<dyn Packetizer>) {
        self.mounts.add(path, packetizer);
    }

    pub fn start(&mut self) -> Result<()> {
        if self.running.load(Ordering::SeqCst) {
            return Err(RtspError::AlreadyRunning);
        }

        let addr: SocketAddr = self
            .bind_addr
            .parse()
            .map_err(|_| RtspError::InvalidBindAddress(format!(
                "expected host:port with explicit port, got {:?}",
                self.bind_addr
            )))?;
        if addr.port() == 0 {
            return Err(RtspError::InvalidBindAddress(
                "port must be explicit (non-zero)".to_string(),
            ));
        }

        self.udp = Some(UdpTransport::bind()?);

        let listener = TcpListener::bind(&self.bind_addr)?;
        listener.set_nonblocking(true)?;

        self.running.store(true, Ordering::SeqCst);

        let running = self.running.clone();
        let session_manager = self.session_manager.clone();
        let mounts = self.mounts.clone();
        let config = self.config.clone();

        tracing::info!(addr = %self.bind_addr, "RTSP server listening");

        thread::spawn(move || {
            tcp::accept_loop(listener, session_manager, mounts, config, running);
        });

        Ok(())
    }

    pub fn stop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        tracing::info!("server stopping");
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Send a raw encoded frame to the default mount (`/stream`).
    ///
    /// Packetizes the data into RTP packets and delivers them to all
    /// subscribed playing sessions via UDP.
    pub fn send_frame(&self, data: &[u8], timestamp_increment: u32) -> Result<usize> {
        self.send_frame_to(DEFAULT_MOUNT_PATH, data, timestamp_increment)
    }

    /// Send a raw encoded frame to a specific mount.
    ///
    /// Packetizes the data using the mount's codec and delivers the
    /// resulting RTP packets to all subscribed playing sessions.
    pub fn send_frame_to(
        &self,
        mount_path: &str,
        data: &[u8],
        timestamp_increment: u32,
    ) -> Result<usize> {
        let udp = self.udp.as_ref().ok_or(RtspError::NotStarted)?;
        let mount = self
            .mounts
            .get(mount_path)
            .ok_or_else(|| RtspError::MountNotFound(mount_path.to_string()))?;

        let packets = mount.packetize(data, timestamp_increment);
        let session_ids = mount.subscribed_session_ids();

        let mut sent = 0;
        for session_id in &session_ids {
            let session = match self.session_manager.get_session(session_id) {
                Some(s) if s.is_playing() => s,
                _ => continue,
            };
            let transport = match session.get_transport() {
                Some(t) => t,
                None => continue,
            };
            for packet in &packets {
                match udp.send_to(packet, transport.client_addr) {
                    Ok(_) => {}
                    Err(e) => {
                        tracing::warn!(
                            session_id,
                            addr = %transport.client_addr,
                            error = %e,
                            "failed to send RTP packet"
                        );
                    }
                }
            }
            sent += 1;
        }

        Ok(sent)
    }

    /// Send a pre-packetized RTP packet to a specific session.
    pub fn send_rtp_packet(&self, session_id: &str, payload: &[u8]) -> Result<usize> {
        let udp = self.udp.as_ref().ok_or(RtspError::NotStarted)?;
        let session = self
            .session_manager
            .get_session(session_id)
            .ok_or_else(|| RtspError::SessionNotFound(session_id.to_string()))?;
        if !session.is_playing() {
            return Err(RtspError::SessionNotPlaying(session_id.to_string()));
        }
        let transport = session
            .get_transport()
            .ok_or_else(|| RtspError::TransportNotConfigured(session_id.to_string()))?;
        udp.send_to(payload, transport.client_addr)
    }

    /// Broadcast a pre-packetized RTP packet to all playing sessions
    /// on the default mount.
    pub fn broadcast_rtp_packet(&self, payload: &[u8]) -> Result<usize> {
        let udp = self.udp.as_ref().ok_or(RtspError::NotStarted)?;
        let mount = self
            .mounts
            .get(DEFAULT_MOUNT_PATH)
            .ok_or_else(|| RtspError::MountNotFound(DEFAULT_MOUNT_PATH.to_string()))?;

        let session_ids = mount.subscribed_session_ids();
        let mut sent = 0;
        for session_id in &session_ids {
            let session = match self.session_manager.get_session(session_id) {
                Some(s) if s.is_playing() => s,
                _ => continue,
            };
            if let Some(transport) = session.get_transport() {
                match udp.send_to(payload, transport.client_addr) {
                    Ok(_) => sent += 1,
                    Err(e) => {
                        tracing::warn!(
                            session_id,
                            addr = %transport.client_addr,
                            error = %e,
                            "failed to send RTP packet"
                        );
                    }
                }
            }
        }
        Ok(sent)
    }

    pub fn get_viewers(&self) -> Vec<Viewer> {
        self.session_manager
            .get_playing_sessions()
            .iter()
            .filter_map(|session| {
                session.get_transport().map(|transport| Viewer {
                    session_id: session.id.clone(),
                    uri: session.uri.clone(),
                    client_addr: transport.client_addr.to_string(),
                    client_rtp_port: transport.client_rtp_port,
                })
            })
            .collect()
    }

    pub fn session_manager(&self) -> &SessionManager {
        &self.session_manager
    }

    /// Returns the mount registry (used by adapters that need mount access).
    pub fn mounts(&self) -> &MountRegistry {
        &self.mounts
    }

    /// Returns the server's protocol configuration.
    pub fn config(&self) -> Arc<ServerConfig> {
        self.config.clone()
    }
}

/// Information about a connected viewer (client in PLAY state).
#[derive(Debug, Clone)]
pub struct Viewer {
    pub session_id: String,
    pub uri: String,
    pub client_addr: String,
    pub client_rtp_port: u16,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_rejects_port_zero() {
        let mut server = Server::new("127.0.0.1:0");
        let err = server.start().unwrap_err();
        match &err {
            RtspError::InvalidBindAddress(msg) => assert!(msg.contains("non-zero"), "{}", msg),
            _ => panic!("expected InvalidBindAddress, got {:?}", err),
        }
    }

    #[test]
    fn start_rejects_missing_port() {
        let mut server = Server::new("127.0.0.1");
        let err = server.start().unwrap_err();
        match &err {
            RtspError::InvalidBindAddress(_) => {}
            _ => panic!("expected InvalidBindAddress, got {:?}", err),
        }
    }

    #[test]
    fn start_accepts_explicit_port() {
        let mut server = Server::new("127.0.0.1:18555");
        server.start().expect("explicit port should be accepted");
        assert!(server.is_running());
        server.stop();
    }
}
