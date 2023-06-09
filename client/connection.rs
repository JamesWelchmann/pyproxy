use std::io::{self, Read, Write};
use std::net::TcpStream as StdTcpStream;

use mio::event::Source;
use mio::net::TcpStream as MioTcpStream;
use mio::{Interest, Registry, Token};
use pyo3::prelude::*;

use protocol::{
    MessageType, RequestClientHello, RequestMessageHeader, ResponseClientHello,
    ResponseMessageHeader,
};

use super::errors::{fatal_io_error, Error, Result};

// Connections must be mio Source and Readers and Writers
pub trait Connection: Source + io::Read + io::Write + Send {}

#[pyclass]
pub struct SimpleConnection {
    stream: MioTcpStream,
}

#[pyclass]
pub struct TlsConnection {
    // TODO
}

#[pyclass]
pub struct PyConnection {
    pub inner: Option<Box<dyn Connection>>,
    pub session_id: String,
    pub stream_token: String,
    pub output_addr: String,
}

#[pymethods]
impl PyConnection {
    #[getter]
    pub fn session_id(&self) -> &str {
        &self.session_id[..]
    }
}

#[pyfunction]
pub fn new_simple_connection(addr: &str) -> Result<PyConnection> {
    let mut stream = fatal_io_error(
        "failed to open TCP stream to PyProxy Server",
        StdTcpStream::connect(addr),
    )?;

    // Send a client hello to server
    let payload = RequestClientHello::new().into_buf();
    let header = RequestMessageHeader::new(MessageType::Hello, 0, payload.len()).into_buf();

    // Send client hello to server
    fatal_io_error(
        "failed to write client hello on mainstream to open TCP Stream with PyProxy server",
        stream
            .write_all(&header)
            .and_then(|_| stream.write_all(&payload)),
    )?;

    // Block - waiting for server response
    let mut header_buf = [0; protocol::RESPONSE_HEADER_SIZE];
    fatal_io_error(
        "reading failed on mainstream with open TCP Stream to PyProxyServer",
        stream.read_exact(&mut header_buf),
    )?;
    let resp_header = ResponseMessageHeader::from_buf(header_buf)?;
    match resp_header.msg_type {
        MessageType::Hello => {}
        _ => return Err(Error::ServerDidntSendHello),
    }

    // Okay read the response payload
    let mut buffer = vec![0; resp_header.msg_len()];
    fatal_io_error(
        "reading failed on mainstream with open TCP Stream to PyProxyServer",
        stream.read_exact(&mut buffer),
    )?;
    let server_hello: ResponseClientHello = protocol::read_msg(&buffer)?;

    fatal_io_error(
        "setting mainstream open socket to non-blocking failed",
        stream.set_nonblocking(true),
    )?;

    Ok(PyConnection {
        inner: Some(Box::new(SimpleConnection {
            stream: MioTcpStream::from_std(stream),
        })),
        session_id: server_hello.session_id,
        stream_token: server_hello.stream_token,
        output_addr: server_hello.output_addr,
    })
}

impl Source for SimpleConnection {
    fn register(
        &mut self,
        registery: &Registry,
        token: Token,
        interest: Interest,
    ) -> io::Result<()> {
        Source::register(&mut self.stream, registery, token, interest)
    }

    fn reregister(
        &mut self,
        registery: &Registry,
        token: Token,
        interest: Interest,
    ) -> io::Result<()> {
        Source::reregister(&mut self.stream, registery, token, interest)
    }

    fn deregister(&mut self, registery: &Registry) -> io::Result<()> {
        Source::deregister(&mut self.stream, registery)
    }
}

impl io::Read for SimpleConnection {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        io::Read::read(&mut self.stream, buf)
    }
}

impl io::Write for SimpleConnection {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        io::Write::write(&mut self.stream, data)
    }

    fn flush(&mut self) -> io::Result<()> {
        io::Write::flush(&mut self.stream)
    }
}

impl Connection for SimpleConnection {}
