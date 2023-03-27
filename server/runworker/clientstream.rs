use std::collections::VecDeque;
use std::io::{self, Read, Write};

use mio::event::Source;
use mio::net::TcpStream;
use mio::{Interest, Registry, Token};
use rand::Rng;

use crate::config::Config;

use super::errors::{io_error, Error, Result};

pub struct ClientStream {
    stream: TcpStream,
    outbuffer: Vec<u8>,
    inbuffer: Vec<u8>,
    interest: Interest,
    session_id: String,
    seq_num: u32,
    req_msgs: VecDeque<protocol::RequestMessage>,
}

impl ClientStream {
    pub fn new(
        cfg: &Config,
        header: [u8; protocol::REQUEST_HEADER_SIZE],
        stream: TcpStream,
        interest: Interest,
    ) -> protocol::Result<Self> {
        let msg_header = protocol::RequestMessageHeader::from_buf(header)?;

        // Check it's client hello
        match msg_header.msg_type {
            protocol::MessageType::Hello => {}
            _ => return Err(protocol::Error::UnexpectedMessageType(msg_header.msg_type)),
        }

        // Okay - populate our buffer with
        let session_id: [u8; 16] = rand::thread_rng().gen();
        let stream_token: [u8; 32] = rand::thread_rng().gen();

        let server_hello = bincode::serialize(&protocol::ResponseClientHello {
            session_id: hex::encode(&session_id),
            stream_token: hex::encode(&stream_token),
            output_addr: cfg.output_addr.to_string(),
        })
        .expect("couldn't serialize ResponseClientHello");

        let resp_header = protocol::ResponseMessageHeader::new(
            protocol::MessageType::Hello,
            0,
            server_hello.len(),
            1,
        )
        .into_buf();

        let mut outbuffer = Vec::with_capacity(4096);
        outbuffer.extend(&resp_header);
        outbuffer.extend(&server_hello);

        Ok(Self {
            stream,
            outbuffer,
            inbuffer: Vec::with_capacity(4096),
            interest,
            session_id: hex::encode(&session_id),
            seq_num: 1,
            req_msgs: VecDeque::with_capacity(64),
        })
    }

    pub fn set_interest(&mut self, interest: Interest) {
        self.interest = interest;
    }

    pub fn interest(&self) -> Interest {
        self.interest
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
        let bytes_read = self
            .stream
            .read(buf)
            .map_err(|e| io_error("failed to read from tcp stream", e))?;

        if bytes_read == 0 {
            return Err(Error::StreamClosed);
        }
        self.inbuffer.extend(&buf[..bytes_read]);

        while self.inbuffer.len() >= protocol::REQUEST_HEADER_SIZE {
            let mut header = [0; protocol::REQUEST_HEADER_SIZE];
            for (h, b) in header.iter_mut().zip(self.inbuffer.iter()) {
                *h = *b;
            }

            let req_header = protocol::RequestMessageHeader::from_buf(header)?;

            // Do we have enough bytes?
            if req_header.msg_len() + protocol::REQUEST_HEADER_SIZE < self.inbuffer.len() {
                break;
            }

            let msg_end = req_header.msg_len() + protocol::REQUEST_HEADER_SIZE;

            let msg_body = &self.inbuffer[(protocol::REQUEST_HEADER_SIZE..msg_end)];
            let msg = protocol::read_req(req_header, msg_body)?;
            self.req_msgs.push_back(msg);

            let bytes_remaining = self.inbuffer.len() - msg_end;
            for n in 0..bytes_remaining {
                self.inbuffer[n] = self.inbuffer[n + msg_end];
            }
            self.inbuffer.truncate(bytes_remaining);
        }
        Ok(())
    }

    pub fn has_out_data(&self) -> bool {
        !self.outbuffer.is_empty()
    }

    pub fn next_req_msg(&mut self) -> Option<protocol::RequestMessage> {
        self.req_msgs.pop_front()
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
