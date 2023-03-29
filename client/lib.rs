use pyo3::create_exception;
use pyo3::exceptions::PyException;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;

mod client;
mod connection;
mod errors;
pub use connection::new_simple_connection;
use errors::Error;

create_exception!(
    "pyproxy_client",
    PyProxyError,
    PyException,
    "PyProxyError is the base exception of PyProxy Client"
);

create_exception!(
    "pyproxy_client",
    PyProxyIOError,
    PyProxyError,
    "PyProxyIOError is raised when I/O fails on either mainstream or outputstream"
);

create_exception!(
    "pyproxy_client",
    PyProxyProtocolError,
    PyProxyError,
    concat!(
        "PyProxyProtocolError is raised when client/server protocol sent invalid bytes/messages.",
        "It is used by both mainstream and outputstream"
    )
);

create_exception!(
    "pyproxy_client",
    PyProxyClosedSessionError,
    PyProxyError,
    concat!(
        "PyPRoxyClosedSessionError is raised when we attempt to perform actions ",
        "on a PyProxySession which is already disconnected. ",
        "The most likely reason is you called RemoteProcess.eval on an already closed ",
        "session."
    )
);

create_exception!(
    "pyproxy_client",
    PyProxyRemoteExceptionPickle,
    PyProxyError,
    concat!(
        "PyProxyRemoteExceptionPickle is raised when the executed server code raised an exception ",
        "the embedded data is a pickled exception."
    )
);

create_exception!(
    "pyproxy_client",
    PyProxyFutureTimeout,
    PyProxyError,
    concat!(
        "PyProxyFutureTimeout is raised when we are blocking waiting for a future to compelte ",
        "and we reach the timeout limit."
    )
);

#[pymodule]
fn pyproxy_client(py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<client::PyProxyClient>()?;
    m.add_class::<client::Future>()?;
    m.add_function(wrap_pyfunction!(new_simple_connection, m)?)?;

    m.add("PyProxyError", py.get_type::<PyProxyError>())?;
    m.add("PyProxyIOError", py.get_type::<PyProxyIOError>())?;
    m.add(
        "PyProxyProtocolError",
        py.get_type::<PyProxyProtocolError>(),
    )?;
    m.add(
        "PyProxyClosedSessionError",
        py.get_type::<PyProxyClosedSessionError>(),
    )?;
    m.add(
        "PyProxyRemoteExceptionPickle",
        py.get_type::<PyProxyRemoteExceptionPickle>(),
    )?;
    m.add(
        "PyProxyFutureTimeout",
        py.get_type::<PyProxyFutureTimeout>(),
    )?;
    Ok(())
}

impl From<errors::Error> for PyErr {
    fn from(err: errors::Error) -> PyErr {
        match err {
            Error::Io(io_err) => {
                let reason = format!("{} - [{}]", io_err.action, io_err.error);
                PyProxyIOError::new_err(reason)
            }
            Error::ServerDidntSendHello => {
                PyProxyProtocolError::new_err("PyProxy server didn't send server-hello message")
            }
            Error::MissingMainStream => {
                let reason = "attempted to open second PyProxyCliet over already used connection";
                PyRuntimeError::new_err(reason)
            }
            Error::Protocol(proto_err) => PyProxyProtocolError::new_err(proto_err.reason()),
            Error::ClientThreadDoesNotExist => {
                PyProxyClosedSessionError::new_err("PyProxySession already closed")
            }
            Error::ThreadClosed(err) => PyProxyClosedSessionError::new_err(format!("{:?}", err)),
            Error::OutputStreamClosed => PyProxyIOError::new_err("output stream closed"),
            Error::MainStreamClosed => PyProxyIOError::new_err("mainstream closed"),
            Error::PythonResultError(data) => PyProxyRemoteExceptionPickle::new_err(data),
            Error::FutureTimeout => {
                PyProxyFutureTimeout::new_err("timed out waiting for future to complete")
            }
        }
    }
}
