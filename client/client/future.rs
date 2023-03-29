use std::sync::mpsc;
use std::time;

use pyo3::prelude::*;
use pyo3::types::PyBytes;

use protocol::mainstream::PythonResult;

use crate::errors::{Error, Result};

#[pyclass]
pub struct Future {
    recv: mpsc::Receiver<PythonResult>,
}

impl Future {
    pub fn new(recv: mpsc::Receiver<PythonResult>) -> Self {
        Self { recv }
    }

    fn wait_no_timeout(&self) -> Result<PythonResult> {
        self.recv
            .recv()
            .map_err(|_| Error::ClientThreadDoesNotExist)
    }

    fn wait_timeout(&self, timeout: time::Duration) -> Result<PythonResult> {
        self.recv.recv_timeout(timeout).or_else(|err| match err {
            mpsc::RecvTimeoutError::Timeout => Err(Error::FutureTimeout),
            mpsc::RecvTimeoutError::Disconnected => Err(Error::ClientThreadDoesNotExist),
        })
    }
}

#[pymethods]
impl Future {
    fn wait(&self, py: Python, timeout: Option<u64>) -> Result<Py<PyBytes>> {
        match timeout {
            Some(t) => self.wait_timeout(time::Duration::from_secs(t)),
            None => self.wait_no_timeout(),
        }
        .and_then(|msg| match msg {
            PythonResult::Error(e) => Err(Error::PythonResultError(e)),
            PythonResult::Return(ret) => {
                let bytes = PyBytes::new(py, &ret).into_py(py);
                Ok(bytes)
            }
        })
    }
}
