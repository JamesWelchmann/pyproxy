use std::sync::mpsc;
use std::thread;

use ndjsonlogger::error;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict};

use protocol::{CodeExec, RequestMessage};

use super::errors::{io_error, Result};

pub fn start() -> Result<(mpsc::Sender<RequestMessage>, mpsc::Receiver<CodeExec>)> {
    let (req_send, req_recv) = mpsc::channel();
    let (exec_send, exec_recv) = mpsc::channel();

    thread::Builder::new()
        .name(String::from("pythread"))
        .spawn(move || Python::with_gil(|py| run_forever(py, req_recv, exec_send)))
        .map_err(|e| io_error("failed to spawn pythread", e))?;

    Ok((req_send, exec_recv))
}

fn run_forever(py: Python, recv: mpsc::Receiver<RequestMessage>, send: mpsc::Sender<CodeExec>) {
    let loads = get_pickle_loads(py).unwrap();
    let dumps = get_pickle_dumps(py).unwrap();

    for msg in recv {
        match msg {
            RequestMessage::Hello(_) => {}
            RequestMessage::CodePickle(p) => err_handler(proc_code_pickle(py, p, &loads)),
            RequestMessage::CodeString(s) => {
                res_handler(py, proc_code_string(py, &s, &loads), &dumps, &send)
            }
        }
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

    println!("locals = {:?}", locals_dict);
    println!("globals = {:?}", globals_dict);

    py.run(&msg.code, Some(&globals_dict), Some(&locals_dict))
        .map(|o| o.into_py(py))
}

fn err_handler(res: PyResult<()>) {}

fn res_handler(
    py: Python,
    res: PyResult<PyObject>,
    dumps: &PyObject,
    send: &mpsc::Sender<CodeExec>,
) {
    println!("res = {:?}", res);

    // Pickle then return value
    let res = res.and_then(|o| dumps.call1(py, (o,)).map(|o| o.into_py(py)));

    match res {
        Ok(bytes) => {
            // Res is a bytes instance returned from pickle.dumps
            let bytes: &PyBytes = bytes.downcast(py).unwrap();
        }
        Err(py_err) => {
            // Log the pyerror
            error!("failed to run python code on pyproxy server", {
                error = &format!("{:?}", py_err)
            });
        }
    }
}
