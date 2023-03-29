use std::collections::HashMap;
use std::io::Write;
use std::net::TcpStream as StdTcpStream;
use std::sync::mpsc;
use std::thread;
use std::time;

use mio::net::TcpStream as MioTcpStream;
use mio::{Events, Interest, Poll, Token};
use pyo3::prelude::*;
use pyo3::types::PyBytes;

use crate::connection::{Connection, PyConnection};

use super::errors::{fatal_io_error, Error, Result};

mod future;
mod mainstream;
mod outputstream;
pub use future::Future;

const MAIN_STREAM_TK: Token = Token(0);
const OUTPUT_STREAM_TK: Token = Token(1);
const RO: Interest = Interest::READABLE;
const POLL_DURATION: time::Duration = time::Duration::from_millis(100);

enum EvalMsg {
    // Python Source Code
    String(String),
}

struct EvalCode {
    id: String,
    msg: EvalMsg,
    locals: Vec<u8>,
    globals: Vec<u8>,
    future_send: mpsc::Sender<protocol::mainstream::PythonResult>,
}

enum ThreadMsg {
    PipeOut(outputstream::PipeOut),
}

#[pyclass]
pub struct PyProxyClient {
    handle: Option<thread::JoinHandle<Result<()>>>,
    code_send: mpsc::Sender<EvalCode>,
    thread_recv: mpsc::Receiver<ThreadMsg>,
    close_send: mpsc::Sender<()>,
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
        let (thread_send, thread_recv) = mpsc::channel();
        let (close_send, close_recv) = mpsc::channel();

        let handle = fatal_io_error(
            "PyProxyClient failed to spawn OS thread",
            thread::Builder::new().name(name.to_owned()).spawn(move || {
                run_forever(
                    stream,
                    code_recv,
                    thread_send,
                    close_recv,
                    session_id,
                    stream_token,
                    output_addr,
                )
            }),
        )?;

        Ok(Self {
            handle: Some(handle),
            code_send,
            thread_recv,
            close_send,
        })
    }

    fn check_thread(&mut self) -> Result<()> {
        let mut closed = false;
        match self.handle.as_ref() {
            None => {
                return Err(Error::ClientThreadDoesNotExist);
            }
            Some(handle) => {
                if handle.is_finished() {
                    // Thread has finished
                    closed = true;
                }
            }
        }

        // Thread is still active - check complete
        if !closed {
            return Ok(());
        }

        match self.handle.take().unwrap().join() {
            Err(err) => Err(Error::ThreadClosed(err)),
            Ok(res) => match res {
                Ok(()) => Err(Error::ClientThreadDoesNotExist),
                // Propogate error to main process
                Err(e) => Err(e),
            },
        }
    }

    pub fn eval_str(
        &mut self,
        id: &str,
        code: &str,
        locs: &PyBytes,
        globs: &PyBytes,
    ) -> Result<Future> {
        self.check_thread()?;
        let (future_send, future_recv) = mpsc::channel();

        self.code_send
            .send(EvalCode {
                id: id.to_owned(),
                msg: EvalMsg::String(code.to_owned()),
                locals: Vec::from_iter(locs.as_bytes().iter().map(|b| *b)),
                globals: Vec::from_iter(globs.as_bytes().iter().map(|b| *b)),
                future_send,
            })
            .map_err(|_| {
                Error::ThreadClosed(Box::new("failed to send code to background os thread"))
            })?;

        Ok(Future::new(future_recv))
    }

    pub fn next_output(&mut self, py: Python) -> Result<Option<(usize, Py<PyBytes>)>> {
        match self.thread_recv.try_recv() {
            Ok(ThreadMsg::PipeOut(pipe_frame)) => {
                let fd = match pipe_frame.fd {
                    protocol::outputstream::MessageType::Stdout => 1,
                    protocol::outputstream::MessageType::Stderr => 2,
                };
                let bytes = PyBytes::new(py, &pipe_frame.line).into_py(py);
                Ok(Some((fd, bytes)))
            }
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(mpsc::TryRecvError::Disconnected) => Err(Error::ClientThreadDoesNotExist),
        }
    }

    pub fn disconnect(&self) {
        self.close_send.send(()).unwrap_or(());
    }
}

fn run_forever(
    stream: Box<dyn Connection>,
    code_recv: mpsc::Receiver<EvalCode>,
    thread_send: mpsc::Sender<ThreadMsg>,
    close_recv: mpsc::Receiver<()>,
    _session_id: String,
    stream_token: String,
    output_addr: String,
) -> Result<()> {
    // Connect to logging stream
    let output_stream = connect_output_stream(output_addr, stream_token)?;
    let mut output_stream = outputstream::OutputStream::new(output_stream);

    let mut poll = fatal_io_error("failed to create mio Poll instance", Poll::new())?;
    let mut events = Events::with_capacity(4096);

    let mut main_stream = mainstream::MainStream::new(stream, RO);
    fatal_io_error(
        "failed to register mainstream with mio for polling",
        poll.registry()
            .register(&mut main_stream, MAIN_STREAM_TK, RO),
    )?;

    fatal_io_error(
        "failed to register outputstream with mio for polling",
        poll.registry()
            .register(&mut output_stream, OUTPUT_STREAM_TK, RO),
    )?;

    let mut buffer = vec![0; 4096];
    let mut pending_futures = HashMap::new();

    loop {
        // Have we received a close?
        match close_recv.try_recv() {
            Ok(_) => return Ok(()),
            Err(mpsc::TryRecvError::Disconnected) => return Ok(()),
            Err(mpsc::TryRecvError::Empty) => {}
        }
        // Do we have any new code to send?
        loop {
            match code_recv.try_recv() {
                Ok(msg) => match msg.msg {
                    EvalMsg::String(s) => {
                        pending_futures.insert(msg.id.to_owned(), msg.future_send);
                        main_stream.queue_source_code(msg.id, s, msg.locals, msg.globals);
                    }
                },
                // No code to send
                Err(mpsc::TryRecvError::Empty) => break,
                // Session done
                Err(mpsc::TryRecvError::Disconnected) => return Ok(()),
            }
        }

        while let Some(resp_msg) = main_stream.next_resp_msg() {
            if let Some(sender) = pending_futures.remove(resp_msg.future_id()) {
                match resp_msg {
                    protocol::ResponseMessage::CodePickle(p) => {
                        sender.send(p.py_result).unwrap_or(());
                    }
                    protocol::ResponseMessage::CodeString(p) => {
                        sender.send(p.py_result).unwrap_or(());
                    }
                    _ => {}
                }
            }
        }

        // Do we need to reregister our mainstream?
        if main_stream.interest() == RO && main_stream.has_out_data() {
            main_stream.set_interest(Interest::READABLE | Interest::WRITABLE);
            let i = main_stream.interest();

            fatal_io_error(
                "failed to reregister mainstream RO with mio Poll",
                poll.registry()
                    .reregister(&mut main_stream, MAIN_STREAM_TK, i),
            )?;
        } else if main_stream.interest().is_writable() && !main_stream.has_out_data() {
            main_stream.set_interest(RO);
            let i = main_stream.interest();

            fatal_io_error(
                "failed to reregister mainstream RW with mio Poll",
                poll.registry()
                    .reregister(&mut main_stream, MAIN_STREAM_TK, i),
            )?;
        }

        fatal_io_error(
            "failed to call mio poll",
            poll.poll(&mut events, Some(POLL_DURATION)),
        )?;

        for ev in &events {
            if ev.token() == MAIN_STREAM_TK {
                if ev.is_writable() {
                    fatal_io_error(
                        "failed to write bytes to mainstream session",
                        main_stream.write(),
                    )?;
                }

                main_stream.read(&mut buffer)?;
            } else if ev.token() == OUTPUT_STREAM_TK {
                for pipe_out in output_stream.read(&mut buffer)? {
                    if thread_send.send(ThreadMsg::PipeOut(pipe_out)).is_err() {
                        return Ok(());
                    }
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

    let mut stream = fatal_io_error(
        "failed to open TCP Stream for output stream",
        StdTcpStream::connect(&output_addr),
    )?;

    fatal_io_error(
        "failed to write client hello on output stream",
        stream.write_all(&msg).and_then(|_| stream.flush()),
    )?;

    fatal_io_error(
        "failed to set output stream TCP Stream to non-blocking",
        stream.set_nonblocking(true),
    )?;

    Ok(MioTcpStream::from_std(stream))
}
