use std::sync::mpsc;
use std::thread;

use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict};

use protocol::RequestMessage;

use crate::messages::{LogValue, NEW_REQUEST_END, NEW_REQUEST_START};

use super::errors::{io_error, Result};
use super::workerstream::Logger;

pub enum ResponseMessage {}

pub fn start(
    logger: Logger,
) -> Result<(
    mpsc::Sender<(String, RequestMessage)>,
    mpsc::Receiver<ResponseMessage>,
)> {
    let (req_send, req_recv) = mpsc::channel();
    let (exec_send, exec_recv) = mpsc::channel();

    thread::Builder::new()
        .name(String::from("pythread"))
        .spawn(move || Python::with_gil(|py| run_forever(py, logger, req_recv, exec_send)))
        .map_err(|e| io_error("failed to spawn pythread", e))?;

    Ok((req_send, exec_recv))
}

fn run_forever(
    py: Python,
    logger: Logger,
    recv: mpsc::Receiver<(String, RequestMessage)>,
    sender: mpsc::Sender<ResponseMessage>,
) {
    let loads = get_pickle_loads(py).unwrap();
    let dumps = get_pickle_dumps(py).unwrap();

    for (session_id, msg) in recv {
        let mut future_id = String::from("0000");
        logger.print(format!("{}{}", NEW_REQUEST_START, session_id));

        match msg {
            RequestMessage::Hello(_) => {}
            RequestMessage::CodePickle(p) => err_handler(proc_code_pickle(py, p, &loads)),
            RequestMessage::CodeString(s) => {
                future_id = s.future_id.clone();
                res_handler(py, logger.clone(), proc_code_string(py, &s, &loads), &dumps);
            }
        }

        logger.info(
            "finished processing pyproxyatom",
            vec![
                ("session_id", LogValue::String(session_id)),
                ("future_id", LogValue::String(future_id)),
            ],
        );
    }
}

fn get_pickle_loads(py: Python) -> PyResult<PyObject> {
    PyModule::import(py, "pickle")
        .and_then(|m| m.getattr("loads"))
        .map(|f| f.into_py(py))
}

fn get_pickle_dumps(py: Python) -> PyResult<PyObject> {
    PyModule::import(py, "pickle")
        .and_then(|m| m.getattr("dumps"))
        .map(|f| f.into_py(py))
}

fn proc_code_pickle(py: Python, msg: protocol::CodePickle, loads: &PyObject) -> PyResult<()> {
    let locals = loads.call1(py, (msg.locals,))?;
    let globals = loads.call1(py, (msg.globals,))?;
    Ok(())
}

fn proc_code_string(
    py: Python,
    msg: &protocol::CodeString,
    loads: &PyObject,
) -> PyResult<PyObject> {
    let locals = loads.call1(py, (&msg.locals[..],))?;
    let globals = loads.call1(py, (&msg.globals[..],))?;

    let locals_dict: &PyDict = locals.downcast(py)?;
    let globals_dict: &PyDict = globals.downcast(py)?;

    py.run(&msg.code, Some(&globals_dict), Some(&locals_dict))
        .map(|o| o.into_py(py))
}

fn err_handler(res: PyResult<()>) {}

fn res_handler(py: Python, logger: Logger, res: PyResult<PyObject>, dumps: &PyObject) {
    // Pickle then return value
    let res = res.and_then(|o| dumps.call1(py, (o,)).map(|o| o.into_py(py)));

    match res {
        Ok(bytes) => {
            // Res is a bytes instance returned from pickle.dumps
            let bytes: &PyBytes = bytes.downcast(py).unwrap();
        }
        Err(py_err) => {
            // Log the pyerror
            logger.error(
                "failed to run python code on pyproxy server",
                vec![("error", LogValue::String(format!("{}", py_err)))],
            );
        }
    }
}
