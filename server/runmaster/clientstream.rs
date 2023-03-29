use std::io::{self, Read};
use std::os::fd::{AsRawFd, RawFd};

use mio::event::Source;
use mio::net::TcpStream;
use mio::{Interest, Registry, Token};

#[derive(Debug)]
pub struct ClientStream {
    stream: TcpStream,
    buffer: [u8; protocol::REQUEST_HEADER_SIZE],
    bytes_read: usize,
}

#[derive(Debug)]
pub enum ReadResult {
    Continue,
    Closed,
    Error(io::Error),
    Done,
}

impl ClientStream {
    pub fn new(stream: TcpStream) -> Self {
        Self {
            stream,
            buffer: [0; protocol::REQUEST_HEADER_SIZE],
            bytes_read: 0,
        }
    }

    pub fn read(&mut self) -> ReadResult {
        let buf = &mut self.buffer[self.bytes_read..];

        match self.stream.read(buf) {
            Ok(bytes_read) => {
                if bytes_read == 0 {
                    ReadResult::Closed
                } else {
                    self.bytes_read += bytes_read;
                    if self.bytes_read == protocol::REQUEST_HEADER_SIZE {
                        ReadResult::Done
                    } else {
                        ReadResult::Continue
                    }
                }
            }
            Err(io_err) => ReadResult::Error(io_err),
        }
    }

    pub fn header(&self) -> [u8; protocol::REQUEST_HEADER_SIZE] {
        self.buffer
    }

    pub fn raw_fd(&self) -> RawFd {
        self.stream.as_raw_fd()
    }
}

impl Source for ClientStream {
    fn register(
        &mut self,
        registry: &Registry,
        token: Token,
        interests: Interest,
    ) -> io::Result<()> {
        Source::register(&mut self.stream, registry, token, interests)
    }

    fn reregister(
        &mut self,
        registry: &Registry,
        token: Token,
        interests: Interest,
    ) -> io::Result<()> {
        Source::reregister(&mut self.stream, registry, token, interests)
    }

    fn deregister(&mut self, registry: &Registry) -> io::Result<()> {
        Source::deregister(&mut self.stream, registry)
    }
}
