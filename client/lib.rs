use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;

mod client;
mod connection;
mod errors;
pub use connection::new_simple_connection;
use errors::Error;

#[pymodule]
fn pyproxy_client(_: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<client::PyProxyClient>()?;
    m.add_function(wrap_pyfunction!(new_simple_connection, m)?)?;
    Ok(())
}

impl From<errors::Error> for PyErr {
    fn from(err: errors::Error) -> PyErr {
        match err {
            Error::Io(io_err) => io_err.into(),
            Error::ServerDidntSendHello => {
                PyRuntimeError::new_err("server didn't send back hello message")
            }
            Error::Protocol(proto_err) => PyRuntimeError::new_err(format!("{:?}", proto_err)),
        }
    }
}
