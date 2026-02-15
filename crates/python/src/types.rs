use pyo3::prelude::*;

use rtsp::Viewer;

#[pyclass(name = "Viewer", skip_from_py_object)]
#[derive(Clone)]
pub struct PyViewer {
    #[pyo3(get)]
    pub session_id: String,
    #[pyo3(get)]
    pub uri: String,
    #[pyo3(get)]
    pub client_addr: String,
    #[pyo3(get)]
    pub client_rtp_port: u16,
}

impl From<Viewer> for PyViewer {
    fn from(v: Viewer) -> Self {
        PyViewer {
            session_id: v.session_id,
            uri: v.uri,
            client_addr: v.client_addr,
            client_rtp_port: v.client_rtp_port,
        }
    }
}

#[pymethods]
impl PyViewer {
    fn __repr__(&self) -> String {
        format!(
            "Viewer(session_id='{}', uri='{}', client_addr='{}', client_rtp_port={})",
            self.session_id, self.uri, self.client_addr, self.client_rtp_port
        )
    }
}
