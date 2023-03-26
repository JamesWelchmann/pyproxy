use std::io;
use std::net::TcpStream as StdTcpStream;
use std::sync::{Arc, Mutex};
use std::thread;

use mio::net::TcpStream as MioTcpStream;
use mio::{Events, Interest, Poll, Token};
use pyo3::prelude::*;
use pyo3::types::PyDict;

use crate::connection::{Connection, PyConnection};

use super::errors::{Error, Result};

mod mainstream;
mod outputstream;

const STREAM_TK: Token = Token(0);
const OUTPUT_STREAM_TK: Token = Token(1);
const RO: Interest = Interest::READABLE;

#[pyclass]
pub struct MysticClient {
    handle: thread::JoinHandle<Result<()>>,
}

#[pymethods]
impl MysticClient {
    #[new]
    #[pyo3(signature=(conn, name=None))]
    fn new(conn: &PyConnection, name: Option<&str>) -> io::Result<Self> {
        // Spawn background thread
        let name = name.unwrap_or("mytic-client");

        let stream = conn.inner.clone();
        let session_id = conn.session_id.to_owned();
        let stream_token = conn.stream_token.to_owned();
        let output_addr = conn.output_addr.to_owned();

        let handle = thread::Builder::new()
            .name(name.to_owned())
            .spawn(move || run_forever(stream, session_id, stream_token, output_addr))?;

        Ok(Self { handle })
    }

    pub fn eval_str(&self, id: &str, code: &str, locs: &PyDict, globals: &PyDict) -> PyResult<()> {
        Ok(())
    }
}

fn run_forever(
    stream: Arc<Mutex<dyn Connection>>,
    session_id: String,
    stream_token: String,
    output_addr: String,
) -> Result<()> {
    // Connect to logging stream
    let output_stream = StdTcpStream::connect(&output_addr)?;
    output_stream.set_nonblocking(true)?;
    let mut output_stream = MioTcpStream::from_std(output_stream);

    let mut poll = Poll::new()?;
    let mut events = Events::with_capacity(4096);

    let mut stream = stream.lock().expect("couldn't lock stream mutex");

    poll.registry().register(&mut (*stream), STREAM_TK, RO)?;
    poll.registry()
        .register(&mut output_stream, OUTPUT_STREAM_TK, RO)?;

    loop {
        poll.poll(&mut events, None)?;

        for ev in &events {
            if ev.token() == STREAM_TK {
            } else if ev.token() == OUTPUT_STREAM_TK {
            }
        }
    }
}
