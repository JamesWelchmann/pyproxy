use std::io::Write;
use std::net::TcpStream as StdTcpStream;
use std::sync::mpsc;
use std::thread;
use std::time;

use mio::net::TcpStream as MioTcpStream;
use mio::{Events, Interest, Poll, Token};
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict};

use crate::connection::{Connection, PyConnection};

use super::errors::{Error, Result};

mod mainstream;
mod outputstream;

const MAIN_STREAM_TK: Token = Token(0);
const OUTPUT_STREAM_TK: Token = Token(1);
const RO: Interest = Interest::READABLE;
const POLL_DURATION: time::Duration = time::Duration::from_millis(100);

enum EvalMsg {
    // Python Source Code
    String(String),

    // Python Pickle Object
    Pickle(Vec<u8>),
}

struct EvalCode {
    id: String,
    msg: EvalMsg,
    locals: Vec<u8>,
    globals: Vec<u8>,
}

#[pyclass]
pub struct PyProxyClient {
    handle: Option<thread::JoinHandle<Result<()>>>,
    code_send: mpsc::Sender<EvalCode>,
    output_recv: mpsc::Receiver<outputstream::PipeOut>,
}

#[pymethods]
impl PyProxyClient {
    #[new]
    #[pyo3(signature=(conn, name=None))]
    fn new(conn: &mut PyConnection, name: Option<&str>) -> Result<Self> {
        // Spawn background thread
        let name = name.unwrap_or("pyproxy-client");

        let stream = conn.inner.take().ok_or(Error::MissingMainStream)?;
        let session_id = conn.session_id.to_owned();
        let stream_token = conn.stream_token.to_owned();
        let output_addr = conn.output_addr.to_owned();

        let (code_send, code_recv) = mpsc::channel();
        let (output_send, output_recv) = mpsc::channel();

        let handle = thread::Builder::new()
            .name(name.to_owned())
            .spawn(move || {
                run_forever(
                    stream,
                    code_recv,
                    output_send,
                    session_id,
                    stream_token,
                    output_addr,
                )
            })?;

        Ok(Self {
            handle: Some(handle),
            code_send,
            output_recv,
        })
    }

    pub fn eval_str(
        &mut self,
        id: &str,
        code: &str,
        locs: &PyBytes,
        globs: &PyBytes,
    ) -> PyResult<()> {
        let mut closed = false;
        match self.handle.as_ref() {
            None => {
                return Err(PyRuntimeError::new_err(
                    "eval_str called on closed pyproxy session",
                ))
            }
            Some(handle) => {
                if handle.is_finished() {
                    closed = true;
                }
            }
        }

        if closed {
            // See above
            let handle = self.handle.take().unwrap();
            return match handle.join() {
                Ok(res) => match res {
                    Err(e) => Err(PyErr::from(e)),
                    Ok(()) => Err(PyRuntimeError::new_err("pyproy thread stopped gracefully")),
                },
                Err(_) => Err(PyRuntimeError::new_err("pyproy thread crashed")),
            };
        }

        self.code_send
            .send(EvalCode {
                id: id.to_owned(),
                msg: EvalMsg::String(code.to_owned()),
                locals: Vec::from_iter(locs.as_bytes().iter().map(|b| *b)),
                globals: Vec::from_iter(globs.as_bytes().iter().map(|b| *b)),
            })
            // TODO: Custom exception
            .map_err(|_| PyRuntimeError::new_err("PyProxyClient closed"))
    }

    pub fn next_output(&self, py: Python) -> PyResult<Option<(usize, Py<PyBytes>)>> {
        match self.output_recv.try_recv() {
            Ok(pipe_frame) => {
                let fd = match pipe_frame.fd {
                    protocol::outputstream::MessageType::Stdout => 1,
                    protocol::outputstream::MessageType::Stderr => 2,
                };
                let bytes = PyBytes::new(py, &pipe_frame.line).into_py(py);
                Ok(Some((fd, bytes)))
            }
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(mpsc::TryRecvError::Disconnected) => {
                // TODO: Raise an exception to show session closed
                Ok(None)
            }
        }
    }
}

fn run_forever(
    stream: Box<dyn Connection>,
    code_recv: mpsc::Receiver<EvalCode>,
    output_send: mpsc::Sender<outputstream::PipeOut>,
    session_id: String,
    stream_token: String,
    output_addr: String,
) -> Result<()> {
    // Connect to logging stream
    let output_stream = connect_output_stream(output_addr, stream_token)?;
    let mut output_stream = outputstream::OutputStream::new(output_stream);

    let mut poll = Poll::new()?;
    let mut events = Events::with_capacity(4096);

    let mut main_stream = mainstream::MainStream::new(stream, RO);
    poll.registry()
        .register(&mut main_stream, MAIN_STREAM_TK, RO)?;

    poll.registry()
        .register(&mut output_stream, OUTPUT_STREAM_TK, RO)?;

    let mut buffer = vec![0; 4096];

    loop {
        // Do we have any new code to send?
        loop {
            match code_recv.try_recv() {
                Ok(msg) => match msg.msg {
                    EvalMsg::String(s) => {
                        println!("queueing source code");
                        main_stream.queue_source_code(msg.id, s, msg.locals, msg.globals);
                    }
                    EvalMsg::Pickle(p) => {
                        main_stream.queue_pickle(msg.id, p, msg.locals, msg.globals);
                    }
                },
                // No code to send
                Err(mpsc::TryRecvError::Empty) => break,
                // Session done
                Err(mpsc::TryRecvError::Disconnected) => return Ok(()),
            }
        }

        // Do we need to reregister our mainstream?
        if main_stream.interest() == RO && main_stream.has_out_data() {
            main_stream.set_interest(Interest::READABLE | Interest::WRITABLE);
            let i = main_stream.interest();
            poll.registry()
                .reregister(&mut main_stream, MAIN_STREAM_TK, i)?;
        } else if main_stream.interest().is_writable() && !main_stream.has_out_data() {
            main_stream.set_interest(RO);
            let i = main_stream.interest();
            poll.registry()
                .reregister(&mut main_stream, MAIN_STREAM_TK, i)?;
        }

        poll.poll(&mut events, Some(POLL_DURATION))?;

        for ev in &events {
            if ev.token() == MAIN_STREAM_TK {
                if ev.is_writable() {
                    main_stream.write()?;
                }
            } else if ev.token() == OUTPUT_STREAM_TK {
                for pipe_out in output_stream.read(&mut buffer)? {
                    output_send.send(pipe_out).unwrap_or(());
                }
            }
        }
    }
}

fn connect_output_stream(output_addr: String, stream_token: String) -> Result<MioTcpStream> {
    let msg = protocol::new_req(
        protocol::MessageType::Hello,
        0,
        protocol::outputstream::ClientHello { stream_token },
    );

    let mut stream = StdTcpStream::connect(&output_addr)?;
    stream.write_all(&msg)?;
    stream.flush()?;

    stream.set_nonblocking(true)?;
    Ok(MioTcpStream::from_std(stream))
}
