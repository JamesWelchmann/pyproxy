use std::cell::RefCell;
use std::io::{self, Read, Write};
use std::rc::Rc;

use mio::event::Source;
use mio::net::TcpStream;
use mio::{Interest, Registry, Token};

use protocol::outputstream::{new_msg, MessageHeader, MessageType};

use super::errors::{fatal_io_err, Result};

#[derive(Clone, Debug)]
pub struct OutputStream {
    pub token: Token,
    inner: Rc<RefCell<Inner>>,
}

impl OutputStream {
    pub fn new(stream: TcpStream, token: Token, interest: Interest) -> Self {
        Self {
            token,
            inner: Rc::new(RefCell::new(Inner::new(stream, interest))),
        }
    }

    pub fn set_interest(&self, interest: Interest) {
        self.inner.borrow_mut().set_interest(interest);
    }

    pub fn interest(&self) -> Interest {
        self.inner.borrow().interest()
    }

    pub fn read(&self, buf: &mut [u8]) -> Result<()> {
        self.inner.borrow_mut().read(buf)
    }

    pub fn take_session_id(&self) -> Option<String> {
        self.inner.borrow_mut().take_session_id()
    }

    pub fn send_stdout(&self, line: &[u8]) {
        self.inner.borrow_mut().send_stdout(line)
    }

    pub fn send_stderr(&self, line: &[u8]) {
        self.inner.borrow_mut().send_stderr(line)
    }

    pub fn write(&self) -> io::Result<()> {
        self.inner.borrow_mut().write()
    }

    pub fn has_out_data(&self) -> bool {
        self.inner.borrow().has_out_data()
    }
}

#[derive(Debug)]
struct Inner {
    stream: TcpStream,
    interest: Interest,
    inbuffer: Vec<u8>,
    outbuffer: Vec<u8>,
    session_id: Option<String>,
}

impl Inner {
    fn new(stream: TcpStream, interest: Interest) -> Self {
        Self {
            stream,
            interest,
            inbuffer: Vec::with_capacity(64),
            outbuffer: Vec::with_capacity(4096),
            session_id: None,
        }
    }

    fn set_interest(&mut self, interest: Interest) {
        self.interest = interest;
    }

    fn interest(&self) -> Interest {
        self.interest
    }

    fn read(&mut self, buf: &mut [u8]) -> Result<()> {
        let bytes_read = fatal_io_err("failed to read output stream", self.stream.read(buf))?;
        self.inbuffer.extend(&buf[..bytes_read]);

        // Read the header
        while self.inbuffer.len() >= protocol::REQUEST_HEADER_SIZE {
            let mut header_raw = [0; protocol::REQUEST_HEADER_SIZE];
            for (h, b) in header_raw.iter_mut().zip(self.inbuffer.iter()) {
                *h = *b;
            }

            let header = protocol::RequestMessageHeader::from_buf(header_raw)?;
            if header.msg_len() + protocol::REQUEST_HEADER_SIZE < self.inbuffer.len() {
                break;
            }

            let msg_end = header.msg_len() + protocol::REQUEST_HEADER_SIZE;
            let payload = &self.inbuffer[protocol::REQUEST_HEADER_SIZE..msg_end];
            let msg = protocol::outputstream::read_hello(payload)?;

            self.session_id = Some(msg.stream_token);
        }
        Ok(())
    }

    fn write(&mut self) -> io::Result<()> {
        let bytes_written = self.stream.write(&self.outbuffer)?;

        let bytes_remaining = self.outbuffer.len() - bytes_written;
        for n in 0..bytes_remaining {
            self.outbuffer[n] = self.outbuffer[n + bytes_written];
        }
        self.outbuffer.truncate(bytes_remaining);

        Ok(())
    }

    fn take_session_id(&mut self) -> Option<String> {
        self.session_id.take()
    }

    fn send_stdout(&mut self, line: &[u8]) {
        let msg_header = MessageHeader::new(MessageType::Stdout, line.len());
        self.outbuffer.extend(&new_msg(msg_header, line));
    }

    fn send_stderr(&mut self, line: &[u8]) {
        let msg_header = MessageHeader::new(MessageType::Stderr, line.len());
        self.outbuffer.extend(&new_msg(msg_header, line));
    }

    fn has_out_data(&self) -> bool {
        !self.outbuffer.is_empty()
    }
}

impl Source for OutputStream {
    fn register(
        &mut self,
        registry: &Registry,
        token: Token,
        interests: Interest,
    ) -> io::Result<()> {
        let mut inner = self.inner.borrow_mut();
        Source::register(&mut inner.stream, registry, token, interests)
    }

    fn reregister(
        &mut self,
        registry: &Registry,
        token: Token,
        interests: Interest,
    ) -> io::Result<()> {
        let mut inner = self.inner.borrow_mut();
        Source::reregister(&mut inner.stream, registry, token, interests)
    }

    fn deregister(&mut self, registry: &Registry) -> io::Result<()> {
        let mut inner = self.inner.borrow_mut();
        Source::deregister(&mut inner.stream, registry)
    }
}

impl Source for Inner {
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
