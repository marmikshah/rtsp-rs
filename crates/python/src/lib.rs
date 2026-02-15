mod packetizer;
mod server;
mod types;

use pyo3::prelude::*;

#[pymodule]
#[pyo3(name = "rtsp")]
fn rtsp_module(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<server::PyServer>()?;
    m.add_class::<packetizer::PyH264Packetizer>()?;
    m.add_class::<types::PyViewer>()?;
    Ok(())
}
