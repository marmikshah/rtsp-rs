use std::net::TcpListener;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

use parking_lot::Mutex;

use crate::error::{Result, RtspError};
use crate::media::Packetizer;
use crate::media::h264::H264Packetizer;
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
/// Owns the session manager, transport layer, and a default packetizer.
/// Delegates TCP connection handling to [`transport::tcp`] and
/// RTP delivery to [`transport::UdpTransport`].
pub struct Server {
    session_manager: SessionManager,
    running: Arc<AtomicBool>,
    bind_addr: String,
    udp: Option<UdpTransport>,
    packetizer: Arc<Mutex<Box<dyn Packetizer>>>,
    config: Arc<ServerConfig>,
}

impl Server {
    pub fn new(bind_addr: &str) -> Self {
        Self::with_config(bind_addr, ServerConfig::default())
    }

    /// Create a server with custom protocol/SDP configuration.
    pub fn with_config(bind_addr: &str, config: ServerConfig) -> Self {
        Self {
            session_manager: SessionManager::new(),
            running: Arc::new(AtomicBool::new(false)),
            bind_addr: bind_addr.to_string(),
            udp: None,
            packetizer: Arc::new(Mutex::new(Box::new(H264Packetizer::with_random_ssrc(96)))),
            config: Arc::new(config),
        }
    }

    /// Create a server with a custom packetizer (for H.265, etc. in the future).
    pub fn with_packetizer(bind_addr: &str, packetizer: Box<dyn Packetizer>) -> Self {
        Self::with_packetizer_and_config(bind_addr, packetizer, ServerConfig::default())
    }

    /// Create a server with a custom packetizer and protocol/SDP configuration.
    pub fn with_packetizer_and_config(
        bind_addr: &str,
        packetizer: Box<dyn Packetizer>,
        config: ServerConfig,
    ) -> Self {
        Self {
            session_manager: SessionManager::new(),
            running: Arc::new(AtomicBool::new(false)),
            bind_addr: bind_addr.to_string(),
            udp: None,
            packetizer: Arc::new(Mutex::new(packetizer)),
            config: Arc::new(config),
        }
    }

    pub fn start(&mut self) -> Result<()> {
        if self.running.load(Ordering::SeqCst) {
            return Err(RtspError::AlreadyRunning);
        }

        self.udp = Some(UdpTransport::bind()?);

        let listener = TcpListener::bind(&self.bind_addr)?;
        listener.set_nonblocking(true)?;

        self.running.store(true, Ordering::SeqCst);

        let running = self.running.clone();
        let session_manager = self.session_manager.clone();
        let packetizer = self.packetizer.clone();
        let config = self.config.clone();

        tracing::info!(addr = %self.bind_addr, "RTSP server listening");

        thread::spawn(move || {
            tcp::accept_loop(listener, session_manager, packetizer, config, running);
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

    pub fn send_rtp_packet(&self, session_id: &str, payload: &[u8]) -> Result<usize> {
        let udp = self.udp.as_ref().ok_or(RtspError::NotStarted)?;
        udp.send_to_session(&self.session_manager, session_id, payload)
    }

    pub fn broadcast_rtp_packet(&self, payload: &[u8]) -> Result<usize> {
        let udp = self.udp.as_ref().ok_or(RtspError::NotStarted)?;
        udp.broadcast(&self.session_manager, payload)
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

    /// Returns a shared reference to the server's packetizer.
    ///
    /// Used by GStreamer sink and other integrations that need to packetize
    /// frames through the same instance the RTSP handler uses for SDP generation.
    pub fn packetizer(&self) -> Arc<Mutex<Box<dyn Packetizer>>> {
        self.packetizer.clone()
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
