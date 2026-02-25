use crate::protocol::request::RtspRequest;
use crate::protocol::response::RtspResponse;
use crate::protocol::sdp;
use crate::media::Packetizer;
use crate::server::ServerConfig;
use crate::session::{SessionManager, SessionState, Transport};
use crate::session::transport::TransportHeader;
use std::net::SocketAddr;
use std::sync::Arc;
use parking_lot::Mutex;

/// Handles RTSP method requests for a single TCP connection.
///
/// Tracks which sessions were created on this connection so they
/// can be cleaned up when the connection drops.
pub struct MethodHandler {
    session_manager: SessionManager,
    client_addr: SocketAddr,
    packetizer: Arc<Mutex<Box<dyn Packetizer>>>,
    config: Arc<ServerConfig>,
    /// Session IDs created during this connection, for cleanup on disconnect.
    session_ids: Vec<String>,
}

impl MethodHandler {
    pub fn new(
        session_manager: SessionManager,
        client_addr: SocketAddr,
        packetizer: Arc<Mutex<Box<dyn Packetizer>>>,
        config: Arc<ServerConfig>,
    ) -> Self {
        MethodHandler {
            session_manager,
            client_addr,
            packetizer,
            config,
            session_ids: Vec::new(),
        }
    }

    /// Returns session IDs owned by this connection (for cleanup on disconnect).
    pub fn session_ids(&self) -> &[String] {
        &self.session_ids
    }

    pub fn handle(&mut self, request: &RtspRequest) -> RtspResponse {
        let cseq = request.cseq().unwrap_or("0");

        match request.method.as_str() {
            "OPTIONS" => self.handle_options(cseq),
            "DESCRIBE" => self.handle_describe(cseq, &request.uri),
            "SETUP" => self.handle_setup(cseq, request),
            "PLAY" => self.handle_play(cseq, request),
            "PAUSE" => self.handle_pause(cseq, request),
            "TEARDOWN" => self.handle_teardown(cseq, request),
            "GET_PARAMETER" => self.handle_get_parameter(cseq, request),
            _ => {
                tracing::warn!(method = %request.method, %cseq, "unsupported RTSP method");
                RtspResponse::new(501, "Not Implemented").add_header("CSeq", cseq)
            }
        }
    }

    fn handle_options(&self, cseq: &str) -> RtspResponse {
        tracing::debug!(%cseq, "OPTIONS");
        RtspResponse::ok()
            .add_header("CSeq", cseq)
            .add_header(
                "Public",
                "OPTIONS, DESCRIBE, SETUP, PLAY, PAUSE, TEARDOWN, GET_PARAMETER",
            )
    }

    /// Parses host from an RTSP URI (e.g. rtsp://host:8554/path -> host). Falls back to client IP if invalid.
    fn host_from_uri_or_client(&self, uri: &str) -> String {
        if let Some(host) = &self.config.public_host {
            return host.clone();
        }

        if let Some(after_scheme) = uri.strip_prefix("rtsp://").or_else(|| uri.strip_prefix("rtsps://")) {
            let host = after_scheme
                .split('/')
                .next()
                .and_then(|host_port| host_port.split(':').next())
                .unwrap_or("")
                .trim();
            if !host.is_empty() {
                return host.to_string();
            }
        }
        self.client_addr.ip().to_string()
    }

    fn handle_describe(&self, cseq: &str, uri: &str) -> RtspResponse {
        tracing::debug!(%cseq, uri, "DESCRIBE");

        if uri.is_empty() || uri == "*" {
            tracing::warn!(uri, "DESCRIBE for invalid URI");
            return RtspResponse::not_found().add_header("CSeq", cseq);
        }

        let host = self.host_from_uri_or_client(uri);
        let sdp = {
            let guard = self.packetizer.lock();
            sdp::generate_sdp(
                &**guard,
                &host,
                &self.config.sdp_session_id,
                &self.config.sdp_session_version,
                &self.config.sdp_username,
                &self.config.sdp_session_name,
            )
        };

        RtspResponse::ok()
            .add_header("CSeq", cseq)
            .add_header("Content-Type", "application/sdp")
            .add_header("Content-Base", uri)
            .with_body(sdp)
    }

    fn handle_setup(&mut self, cseq: &str, request: &RtspRequest) -> RtspResponse {
        let transport_header = match request.get_header("Transport") {
            Some(t) => t,
            None => {
                tracing::warn!(%cseq, "SETUP missing Transport header");
                return RtspResponse::bad_request().add_header("CSeq", cseq);
            }
        };

        let client_transport = match TransportHeader::parse(transport_header) {
            Some(t) => t,
            None => {
                tracing::warn!(%cseq, transport_header, "SETUP invalid Transport header");
                return RtspResponse::bad_request().add_header("CSeq", cseq);
            }
        };

        let (server_rtp_port, server_rtcp_port) = match self.session_manager.allocate_server_ports()
        {
            Ok(ports) => ports,
            Err(e) => {
                tracing::error!(error = %e, "failed to allocate server ports");
                return RtspResponse::new(500, "Internal Server Error").add_header("CSeq", cseq);
            }
        };

        let session = self.session_manager.create_session(&request.uri);
        let session_id = session.id.clone();
        let client_rtp_addr =
            SocketAddr::new(self.client_addr.ip(), client_transport.client_rtp_port);

        session.set_transport(Transport {
            client_rtp_port: client_transport.client_rtp_port,
            client_rtcp_port: client_transport.client_rtcp_port,
            server_rtp_port,
            server_rtcp_port,
            client_addr: client_rtp_addr,
        });

        self.session_ids.push(session_id.clone());

        tracing::info!(
            session_id,
            uri = %request.uri,
            client_rtp = %client_rtp_addr,
            server_rtp_port,
            "session created via SETUP"
        );

        let transport_response = format!(
            "RTP/AVP;unicast;client_port={}-{};server_port={}-{}",
            client_transport.client_rtp_port,
            client_transport.client_rtcp_port,
            server_rtp_port,
            server_rtcp_port
        );

        RtspResponse::ok()
            .add_header("CSeq", cseq)
            .add_header("Transport", &transport_response)
            .add_header("Session", &session.session_header_value())
    }

    fn handle_play(&mut self, cseq: &str, request: &RtspRequest) -> RtspResponse {
        let session_id = match self.extract_session_id(request) {
            Some(id) => id,
            None => {
                tracing::warn!(%cseq, "PLAY missing Session header");
                return RtspResponse::new(454, "Session Not Found").add_header("CSeq", cseq);
            }
        };

        match self.session_manager.get_session(&session_id) {
            Some(session) => {
                session.set_state(SessionState::Playing);
                tracing::info!(session_id, "session started playing");
                let packetizer = self.packetizer.lock();
                let next_sequence = packetizer.next_sequence();
                let next_rtp_timestamp = packetizer.next_rtp_timestamp();
                let rtp_info = format!("url={};seq={};rtptime={}", session.uri, next_sequence, next_rtp_timestamp);
                RtspResponse::ok()
                    .add_header("CSeq", cseq)
                    .add_header("Session", &session.session_header_value())
                    .add_header("Range", "npt=0.000-")
                    .add_header("RTP-Info", &rtp_info)
            }
            None => {
                tracing::warn!(session_id, "PLAY for unknown session");
                RtspResponse::new(454, "Session Not Found").add_header("CSeq", cseq)
            }
        }
    }

    fn handle_pause(&mut self, cseq: &str, request: &RtspRequest) -> RtspResponse {
        let session_id = match self.extract_session_id(request) {
            Some(id) => id,
            None => {
                tracing::warn!(%cseq, "PAUSE missing Session header");
                return RtspResponse::new(454, "Session Not Found").add_header("CSeq", cseq);
            }
        };

        match self.session_manager.get_session(&session_id) {
            Some(session) => {
                session.set_state(SessionState::Paused);
                tracing::info!(session_id, "session paused");
                RtspResponse::ok()
                    .add_header("CSeq", cseq)
                    .add_header("Session", &session.session_header_value())
            }
            None => {
                tracing::warn!(session_id, "PAUSE for unknown session");
                RtspResponse::new(454, "Session Not Found").add_header("CSeq", cseq)
            }
        }
    }

    fn handle_teardown(&mut self, cseq: &str, request: &RtspRequest) -> RtspResponse {
        let session_id = match self.extract_session_id(request) {
            Some(id) => id,
            None => {
                tracing::warn!(%cseq, "TEARDOWN missing Session header");
                return RtspResponse::new(454, "Session Not Found").add_header("CSeq", cseq);
            }
        };

        match self.session_manager.remove_session(&session_id) {
            Some(_) => {
                self.session_ids.retain(|id| id != &session_id);
                tracing::info!(session_id, "session terminated via TEARDOWN");
                RtspResponse::ok().add_header("CSeq", cseq)
            }
            None => {
                tracing::warn!(session_id, "TEARDOWN for unknown session");
                RtspResponse::new(454, "Session Not Found").add_header("CSeq", cseq)
            }
        }
    }

    /// GET_PARAMETER is used by clients (e.g. VLC) as a keepalive (RFC 2326 §10.8).
    fn handle_get_parameter(&self, cseq: &str, request: &RtspRequest) -> RtspResponse {
        tracing::trace!(%cseq, "GET_PARAMETER keepalive");

        let mut resp = RtspResponse::ok().add_header("CSeq", cseq);

        if let Some(id) = self.extract_session_id(request) {
            if self.session_manager.get_session(&id).is_some() {
                resp = resp.add_header("Session", &id);
            }
        }

        resp
    }

    /// Extract session ID from the Session header.
    /// Handles timeout suffix: "SESSIONID;timeout=60" → "SESSIONID"
    fn extract_session_id(&self, request: &RtspRequest) -> Option<String> {
        request.get_header("Session").map(|s| {
            s.split(';').next().unwrap_or(s).trim().to_string()
        })
    }
}
