use std::collections::VecDeque;
use std::io::{self, Write};

use mio::event::Source;
use mio::{Interest, Registry, Token};

use crate::connection::Connection;
use crate::errors::{fatal_io_error, Error, Result};

pub struct MainStream {
    stream: Box<dyn Connection>,
    outbuffer: Vec<u8>,
    inbuffer: Vec<u8>,
    interest: Interest,
    resp_msgs: VecDeque<protocol::ResponseMessage>,
}

impl MainStream {
    pub fn new(stream: Box<dyn Connection>, interest: Interest) -> Self {
        Self {
            stream,
            outbuffer: Vec::with_capacity(16384),
            inbuffer: Vec::with_capacity(4096),
            interest,
            resp_msgs: VecDeque::with_capacity(128),
        }
    }

    pub fn set_interest(&mut self, interest: Interest) {
        self.interest = interest;
    }

    pub fn interest(&self) -> Interest {
        self.interest
    }

    pub fn has_out_data(&self) -> bool {
        !self.outbuffer.is_empty()
    }

    pub fn next_resp_msg(&mut self) -> Option<protocol::ResponseMessage> {
        self.resp_msgs.pop_front()
    }

    pub fn queue_source_code(
        &mut self,
        id: String,
        code: String,
        locals: Vec<u8>,
        globals: Vec<u8>,
    ) {
        let msg = protocol::CodeString {
            future_id: id,
            code,
            locals,
            globals,
        };

        self.outbuffer.extend(&protocol::new_req(
            protocol::MessageType::CodeString,
            0,
            msg,
        ));
    }

    pub fn write(&mut self) -> io::Result<()> {
        let bytes_written = self.stream.write(&self.outbuffer)?;
        self.stream.flush()?;
        let bytes_remaining = self.outbuffer.len() - bytes_written;

        for n in 0..bytes_remaining {
            self.outbuffer[n] = self.outbuffer[n + bytes_written];
        }

        self.outbuffer.truncate(bytes_remaining);
        Ok(())
    }

    pub fn read(&mut self, buf: &mut [u8]) -> Result<()> {
        let bytes_read =
            fatal_io_error("couldn't read on mainstream session", self.stream.read(buf))?;

        if bytes_read == 0 {
            return Err(Error::MainStreamClosed);
        }

        self.inbuffer.extend(&buf[..bytes_read]);

        while self.inbuffer.len() >= protocol::RESPONSE_HEADER_SIZE {
            let mut header_raw = [0; protocol::RESPONSE_HEADER_SIZE];
            for (h, b) in header_raw.iter_mut().zip(self.inbuffer.iter()) {
                *h = *b;
            }

            let header = protocol::ResponseMessageHeader::from_buf(header_raw)?;
            let msg_end = protocol::RESPONSE_HEADER_SIZE + header.msg_len();

            if self.inbuffer.len() < msg_end {
                break;
            }

            let body = &self.inbuffer[protocol::RESPONSE_HEADER_SIZE..msg_end];
            let msg = protocol::read_response(header, body)?;
            if msg.is_hello() {
                Err(protocol::Error::UnexpectedMessageType(
                    protocol::MessageType::Hello,
                ))?;
            }

            self.resp_msgs.push_back(msg);

            let bytes_remaining = self.inbuffer.len() - msg_end;
            for n in 0..bytes_remaining {
                self.inbuffer[n] = self.inbuffer[n + msg_end];
            }
            self.inbuffer.truncate(bytes_remaining);
        }

        Ok(())
    }
}

impl Source for MainStream {
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
