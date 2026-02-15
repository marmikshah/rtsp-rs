//! RTSP session management (RFC 2326 §3, §12.37).
//!
//! An RTSP session is a server-side state object created during SETUP and
//! destroyed by TEARDOWN (or timeout). It tracks:
//!
//! - A unique session ID (hex string, returned in the `Session` header).
//! - The playback state: Ready -> Playing <-> Paused.
//! - Transport parameters (client/server UDP ports) negotiated during SETUP.
//! - A timeout (default 60s, per RFC 2326 §12.37) — the client must send
//!   a request (e.g. GET_PARAMETER) before the timeout expires.
//!
//! ## Session lifecycle (RFC 2326 §A.1)
//!
//! ```text
//! SETUP         -> Ready
//! PLAY          -> Playing
//! PAUSE         -> Paused   (from Playing)
//! PLAY          -> Playing  (from Paused)
//! TEARDOWN      -> (removed)
//! TCP disconnect -> (removed, via cleanup)
//! ```

pub mod transport;

use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::error::Result;
pub use transport::Transport;

static SESSION_COUNTER: AtomicU64 = AtomicU64::new(0);

const SERVER_PORT_MIN: u64 = 5000;
const SERVER_PORT_MAX: u64 = 65534;

/// Default session timeout in seconds (RFC 2326 §12.37).
pub const DEFAULT_SESSION_TIMEOUT_SECS: u64 = 60;

/// RTSP session state machine (RFC 2326 §A.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionState {
    /// Session created via SETUP, not yet playing.
    Ready,
    /// Media is being delivered (RTP packets sent to client).
    Playing,
    /// Delivery suspended; can resume via PLAY.
    Paused,
}

/// A single RTSP session (RFC 2326 §3).
///
/// Created during SETUP, destroyed by TEARDOWN or TCP disconnect.
/// Interior mutability via `RwLock` allows shared references across threads.
#[derive(Debug)]
pub struct Session {
    /// Unique session identifier (16-char hex string).
    pub id: String,
    /// The RTSP URI this session was created for (from the SETUP request).
    pub uri: String,
    /// Transport parameters negotiated during SETUP (RFC 2326 §12.39).
    pub transport: RwLock<Option<Transport>>,
    /// Current playback state.
    pub state: RwLock<SessionState>,
    /// Session timeout in seconds (included in the `Session` response header).
    pub timeout_secs: u64,
}

impl Session {
    /// Create a new session with a unique auto-incrementing ID.
    pub fn new(uri: &str) -> Self {
        let id = SESSION_COUNTER.fetch_add(1, Ordering::SeqCst);
        Session {
            id: format!("{:016X}", id),
            uri: uri.to_string(),
            transport: RwLock::new(None),
            state: RwLock::new(SessionState::Ready),
            timeout_secs: DEFAULT_SESSION_TIMEOUT_SECS,
        }
    }

    /// Set the transport parameters (called during SETUP).
    pub fn set_transport(&self, transport: Transport) {
        tracing::debug!(session_id = %self.id, client_addr = %transport.client_addr, "transport configured");
        *self.transport.write() = Some(transport);
    }

    /// Returns a clone of the transport parameters, if configured.
    pub fn get_transport(&self) -> Option<Transport> {
        self.transport.read().clone()
    }

    /// Transition to a new playback state.
    pub fn set_state(&self, state: SessionState) {
        tracing::debug!(session_id = %self.id, old_state = ?*self.state.read(), new_state = ?state, "state transition");
        *self.state.write() = state;
    }

    /// Returns the current playback state.
    pub fn get_state(&self) -> SessionState {
        self.state.read().clone()
    }

    /// Whether this session is actively receiving media.
    pub fn is_playing(&self) -> bool {
        *self.state.read() == SessionState::Playing
    }

    /// Format the `Session` response header value per RFC 2326 §12.37.
    ///
    /// Example: `"0000000000000001;timeout=60"`
    pub fn session_header_value(&self) -> String {
        format!("{};timeout={}", self.id, self.timeout_secs)
    }
}

/// Thread-safe registry of active sessions.
///
/// Backed by `parking_lot::RwLock` for fast concurrent reads. Session
/// lookups happen on every RTP delivery cycle, so read performance matters.
#[derive(Clone)]
pub struct SessionManager {
    sessions: Arc<RwLock<HashMap<String, Arc<Session>>>>,
    next_server_port: Arc<AtomicU64>,
}

impl SessionManager {
    pub fn new() -> Self {
        SessionManager {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            next_server_port: Arc::new(AtomicU64::new(SERVER_PORT_MIN)),
        }
    }

    /// Create a new session for the given URI and register it.
    pub fn create_session(&self, uri: &str) -> Arc<Session> {
        let session = Arc::new(Session::new(uri));
        let id = session.id.clone();
        self.sessions.write().insert(id.clone(), session.clone());

        let total = self.sessions.read().len();
        tracing::debug!(session_id = %id, uri, total_sessions = total, "session created");

        session
    }

    /// Look up a session by ID.
    pub fn get_session(&self, id: &str) -> Option<Arc<Session>> {
        self.sessions.read().get(id).cloned()
    }

    /// Remove and return a session by ID (used by TEARDOWN).
    pub fn remove_session(&self, id: &str) -> Option<Arc<Session>> {
        let removed = self.sessions.write().remove(id);
        if removed.is_some() {
            let total = self.sessions.read().len();
            tracing::debug!(session_id = %id, total_sessions = total, "session removed");
        }
        removed
    }

    /// Remove multiple sessions at once (used during TCP disconnect cleanup).
    pub fn remove_sessions(&self, ids: &[String]) -> usize {
        let mut sessions = self.sessions.write();
        let mut removed = 0;
        for id in ids {
            if sessions.remove(id).is_some() {
                removed += 1;
            }
        }
        if removed > 0 {
            tracing::debug!(removed, remaining = sessions.len(), "batch session cleanup");
        }
        removed
    }

    /// Allocate a pair of (RTP, RTCP) server ports.
    ///
    /// Ports are allocated from a monotonic counter starting at 5000.
    /// When the range is exhausted (> 65534), it wraps back to 5000.
    /// Per RFC 3550 §11, RTP ports should be even and RTCP = RTP + 1.
    pub fn allocate_server_ports(&self) -> Result<(u16, u16)> {
        let rtp = self.next_server_port.fetch_add(2, Ordering::SeqCst);

        if rtp > SERVER_PORT_MAX {
            tracing::warn!(rtp, "port range exhausted, wrapping to {SERVER_PORT_MIN}");
            self.next_server_port
                .store(SERVER_PORT_MIN, Ordering::SeqCst);
            let rtp = self.next_server_port.fetch_add(2, Ordering::SeqCst);
            return Ok((rtp as u16, rtp as u16 + 1));
        }

        tracing::trace!(
            rtp_port = rtp,
            rtcp_port = rtp + 1,
            "allocated server ports"
        );
        Ok((rtp as u16, rtp as u16 + 1))
    }

    /// Returns all sessions currently in the [`SessionState::Playing`] state.
    pub fn get_playing_sessions(&self) -> Vec<Arc<Session>> {
        self.sessions
            .read()
            .values()
            .filter(|s| s.is_playing())
            .cloned()
            .collect()
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}
