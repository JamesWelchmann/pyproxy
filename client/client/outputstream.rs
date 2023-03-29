use std::io::{self, Read};

use mio::event::Source;
use mio::net::TcpStream;
use mio::{Interest, Registry, Token};

use protocol::outputstream::{MessageHeader, HEADER_SIZE};

use crate::errors::{Error, Result, fatal_io_error};

pub struct OutputStream {
    stream: TcpStream,
    buffer: Vec<u8>,
}

pub struct PipeOut {
    pub line: Vec<u8>,
    pub fd: protocol::outputstream::MessageType,
}

impl OutputStream {
    pub fn new(stream: TcpStream) -> Self {
        Self {
            stream,
            buffer: Vec::with_capacity(4096),
        }
    }

    pub fn read(&mut self, buf: &mut [u8]) -> Result<Vec<PipeOut>> {
        let bytes_read = fatal_io_error(
          "failed to read bytes on outputstream",
          self.stream.read(buf),
        )?;

        if bytes_read == 0 {
          return Err(Error::OutputStreamClosed);
        }

        self.buffer.extend(&buf[..bytes_read]);

        let mut lines = vec![];

        while self.buffer.len() >= HEADER_SIZE {
            let mut header_raw = [0; HEADER_SIZE];
            for (h, b) in header_raw.iter_mut().zip(self.buffer.iter()) {
                *h = *b;
            }

            let header = MessageHeader::from_raw(header_raw)?;
            let body_end = HEADER_SIZE + header.msg_len;
            if self.buffer.len() < body_end {
                return Ok(lines);
            }

            let body = &self.buffer[HEADER_SIZE..body_end];
            lines.push(PipeOut {
                fd: header.msg_type,
                line: body.to_owned(),
            });

            let bytes_remaining = self.buffer.len() - body_end;
            for n in 0..bytes_remaining {
                self.buffer[n] = self.buffer[n + body_end];
            }
            self.buffer.truncate(bytes_remaining);
        }

        Ok(lines)
    }
}

impl Source for OutputStream {
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
