use parking_lot::Mutex;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use std::sync::Arc;

use crate::types::PyViewer;
use rtsp::{Server, ServerConfig};

#[pyclass(name = "Server")]
pub struct PyServer {
    inner: Arc<Mutex<Server>>,
}

impl PyServer {
    fn with_server<F, R>(&self, f: F) -> PyResult<R>
    where
        F: FnOnce(&mut Server) -> R,
    {
        Ok(f(&mut self.inner.lock()))
    }
}

#[pymethods]
impl PyServer {
    #[new]
    #[pyo3(signature = (
        bind_addr = "0.0.0.0:8554",
        public_host = None,
        public_port = None,
        session_name = "Stream",
    ))]
    fn new(
        bind_addr: &str,
        public_host: Option<&str>,
        public_port: Option<u16>,
        session_name: &str,
    ) -> Self {
        let config = ServerConfig {
            public_host: public_host.map(std::string::ToString::to_string),
            public_port,
            sdp_session_name: session_name.to_string(),
            ..ServerConfig::default()
        };
        PyServer {
            inner: Arc::new(Mutex::new(Server::with_config(bind_addr, config))),
        }
    }

    fn start(&self) -> PyResult<()> {
        self.inner
            .lock()
            .start()
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    fn stop(&self) -> PyResult<()> {
        self.with_server(|s| s.stop())
    }

    fn is_running(&self) -> PyResult<bool> {
        self.with_server(|s| s.is_running())
    }

    /// Send a raw encoded frame to the default mount (`/stream`).
    /// Handles packetization and delivery internally.
    fn send_frame(&self, data: &[u8], timestamp_increment: u32) -> PyResult<usize> {
        self.inner
            .lock()
            .send_frame(data, timestamp_increment)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// Send a raw encoded frame to a specific mount.
    fn send_frame_to(
        &self,
        mount_path: &str,
        data: &[u8],
        timestamp_increment: u32,
    ) -> PyResult<usize> {
        self.inner
            .lock()
            .send_frame_to(mount_path, data, timestamp_increment)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// Send a pre-packetized RTP packet to a specific session.
    fn send_rtp_packet(&self, session_id: &str, payload: &[u8]) -> PyResult<usize> {
        self.inner
            .lock()
            .send_rtp_packet(session_id, payload)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// Broadcast a pre-packetized RTP packet to all playing sessions
    /// on the default mount.
    fn broadcast_rtp_packet(&self, payload: &[u8]) -> PyResult<usize> {
        self.inner
            .lock()
            .broadcast_rtp_packet(payload)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    fn get_viewers(&self) -> PyResult<Vec<PyViewer>> {
        let viewers = self.inner.lock().get_viewers();
        Ok(viewers.into_iter().map(PyViewer::from).collect())
    }
}
