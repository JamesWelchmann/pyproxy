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

        let handle = thread::Builder::new()
            .name(name.to_owned())
            .spawn(move || run_forever(stream, code_recv, session_id, stream_token, output_addr))?;

        Ok(Self {
            handle: Some(handle),
            code_send,
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
                    println!("closed = true");
                }
            }
        }

        if closed {
            // See above
            let handle = self.handle.take().unwrap();
            println!("handle = {:?}", handle);
            return match handle.join() {
                Ok(res) => match res {
                    Err(e) => Err(PyErr::from(e)),
                    Ok(()) => Err(PyRuntimeError::new_err("pyproy thread stopped gracefully")),
                },
                Err(_) => {
                    println!("handle error");
                    Err(PyRuntimeError::new_err("pyproy thread crashed"))
                }
            };
        }

        println!("code going to server {}", code);

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
}

fn run_forever(
    stream: Box<dyn Connection>,
    code_recv: mpsc::Receiver<EvalCode>,
    session_id: String,
    stream_token: String,
    output_addr: String,
) -> Result<()> {
    println!("run_forever started");

    // Connect to logging stream
    let output_stream = StdTcpStream::connect(&output_addr)?;
    output_stream.set_nonblocking(true)?;
    let mut output_stream = MioTcpStream::from_std(output_stream);

    println!("got output stream");

    let mut poll = Poll::new()?;
    let mut events = Events::with_capacity(4096);

    println!("created poll");

    let mut main_stream = mainstream::MainStream::new(stream, RO);
    poll.registry()
        .register(&mut main_stream, MAIN_STREAM_TK, RO)?;

    poll.registry()
        .register(&mut output_stream, OUTPUT_STREAM_TK, RO)?;

    println!("pyproxy thread started");

    loop {
        // Do we have any new code to send?
        loop {
            match code_recv.try_recv() {
                Ok(msg) => match msg.msg {
                    EvalMsg::String(s) => {
                        println!("thread got code {}", s);
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

        println!("mainstream.interest = {:?}", main_stream.interest());
        println!("mainstream.has_out_data = {:?}", main_stream.has_out_data());

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
                    println!("thread writing source code");
                    main_stream.write()?;
                }
            } else if ev.token() == OUTPUT_STREAM_TK {
            }
        }
    }
}
