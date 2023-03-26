use std::io::{self, Write};

use mio::event::Source;
use mio::net::TcpStream;
use mio::{Interest, Registry, Token};
use rand::Rng;

use crate::config::Config;

pub struct ClientStream {
    stream: TcpStream,
    outbuffer: Vec<u8>,
    inbuffer: Vec<u8>,
    interest: Interest,
    session_id: String,
    seq_num: u32,
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
