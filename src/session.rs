use parking_lot::RwLock;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

static SESSION_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone)]
pub struct Transport {
    pub client_rtp_port: u16,
    pub client_rtcp_port: u16,
    pub server_rtp_port: u16,
    pub server_rtcp_port: u16,
    pub client_addr: SocketAddr,
}

#[derive(Debug, Clone)]
pub enum PlaybackState {
    Ready,
    Playing,
    Paused,
}

#[derive(Debug)]
pub struct Session {
    pub id: String,
    pub uri: String,
    pub transport: RwLock<Option<Transport>>,
    pub state: RwLock<PlaybackState>,
}

impl Session {
    pub fn new(uri: &str) -> Self {
        let id = SESSION_COUNTER.fetch_add(1, Ordering::SeqCst);
        Session {
            id: format!("{:016X}", id),
            uri: uri.to_string(),
            transport: RwLock::new(None),
            state: RwLock::new(PlaybackState::Ready),
        }
    }

    pub fn set_transport(&self, transport: Transport) {
        *self.transport.write() = Some(transport);
    }

    pub fn get_transport(&self) -> Option<Transport> {
        self.transport.read().clone()
    }

    pub fn set_state(&self, state: PlaybackState) {
        *self.state.write() = state;
    }

    pub fn get_state(&self) -> PlaybackState {
        self.state.read().clone()
    }

    pub fn is_playing(&self) -> bool {
        matches!(*self.state.read(), PlaybackState::Playing)
    }
}

#[derive(Clone)]
pub struct SessionManager {
    sessions: Arc<RwLock<HashMap<String, Arc<Session>>>>,
    next_server_port: Arc<AtomicU64>,
}

impl SessionManager {
    pub fn new() -> Self {
        SessionManager {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            next_server_port: Arc::new(AtomicU64::new(5000)),
        }
    }
    pub fn create_session(&mut self, uri: &str) -> Arc<Session> {
        let session = Arc::new(Session::new(uri));
        let id = session.id.clone();
        self.sessions.write().insert(id, session.clone());
        session
    }

    pub fn get_session(&self, id: &str) -> Option<Arc<Session>> {
        self.sessions.read().get(id).cloned()
    }

    pub fn remove_session(&mut self, id: &str) -> Option<Arc<Session>> {
        self.sessions.write().remove(id)
    }

    pub fn allocate_server_ports(&self) -> (u16, u16) {
        let rtp = self.next_server_port.fetch_add(2, Ordering::SeqCst) as u16;
        (rtp, rtp + 1)
    }

    pub fn get_all_sessions(&self) -> Vec<Arc<Session>> {
        self.sessions.read().values().cloned().collect()
    }

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
