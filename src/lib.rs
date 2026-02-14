use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use std::sync::{Arc, Mutex};

mod handler;
mod protocol;
pub mod server;
mod session;

use server::{RtspServer, ClientInfo};

#[pyclass]
struct PyRtspServer {
    inner: Arc<Mutex<RtspServer>>,
}

#[pymethods]
impl PyRtspServer {
    #[new]
    #[pyo3(signature = (bind_addr = "0.0.0.0:8554"))]
    fn new(bind_addr: &str) -> Self {
        PyRtspServer {
            inner: Arc::new(Mutex::new(RtspServer::new(bind_addr))),
        }
    }

    fn start(&self) -> PyResult<()> {
        self.inner
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?
            .start()
            .map_err(|e| PyRuntimeError::new_err(e))
    }

    fn stop(&self) -> PyResult<()> {
        self.inner
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?
            .stop();
        Ok(())
    }

    fn is_running(&self) -> PyResult<bool> {
        Ok(self
            .inner
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?
            .is_running())
    }

    fn send_rtp_packet(&self, session_id: &str, payload: &[u8]) -> PyResult<usize> {
        self.inner
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?
            .send_rtp_packet(session_id, payload)
            .map_err(|e| PyRuntimeError::new_err(e))
    }

    fn broadcast_rtp_packet(&self, payload: &[u8]) -> PyResult<usize> {
        self.inner
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?
            .broadcast_rtp_packet(payload)
            .map_err(|e| PyRuntimeError::new_err(e))
    }

    fn get_playing_clients(&self) -> PyResult<Vec<PyClientInfo>> {
        let clients = self
            .inner
            .lock()
            .map_err(|e| PyRuntimeError::new_err(format!("Lock error: {}", e)))?
            .get_playing_clients();

        Ok(clients.into_iter().map(PyClientInfo::from).collect())
    }
}

#[pyclass(skip_from_py_object)]
#[derive(Clone)]
struct PyClientInfo {
    #[pyo3(get)]
    session_id: String,
    #[pyo3(get)]
    uri: String,
    #[pyo3(get)]
    client_addr: String,
    #[pyo3(get)]
    client_rtp_port: u16,
}

impl From<ClientInfo> for PyClientInfo {
    fn from(info: ClientInfo) -> Self {
        PyClientInfo {
            session_id: info.session_id,
            uri: info.uri,
            client_addr: info.client_addr,
            client_rtp_port: info.client_rtp_port,
        }
    }
}

#[pymethods]
impl PyClientInfo {
    fn __repr__(&self) -> String {
        format!(
            "ClientInfo(session_id='{}', uri='{}', client_addr='{}', client_rtp_port='{})",
            self.session_id, self.uri, self.client_addr, self.client_rtp_port
        )
    }
}

#[pymodule]
fn rtsp(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyRtspServer>()?;
    m.add_class::<PyClientInfo>()?;
    Ok(())
}
