use std::io::{self, Write};

use mio::event::Source;
use mio::{Interest, Registry, Token};
use pyo3::types::PyDict;
use pyo3::{Py, PyObject};

use crate::connection::Connection;

pub struct MainStream {
    stream: Box<dyn Connection>,
    outbuffer: Vec<u8>,
    interest: Interest,
}

impl MainStream {
    pub fn new(stream: Box<dyn Connection>, interest: Interest) -> Self {
        Self {
            stream,
            outbuffer: Vec::with_capacity(16384),
            interest,
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

    pub fn queue_pickle(&mut self, id: String, pickle: Vec<u8>, locals: Vec<u8>, globals: Vec<u8>) {
        let msg = protocol::CodePickle {
            future_id: id,
            pickle,
            locals,
            globals,
        };

        self.outbuffer.extend(&protocol::new_req(
            protocol::MessageType::CodePickle,
            0,
            msg,
        ));
    }

    pub fn write(&mut self) -> io::Result<()> {
        let bytes_written = self.stream.write(&self.outbuffer)?;
        self.stream.flush()?;
        let bytes_remaining = self.outbuffer.len() - bytes_written;

        println!("wrote {} bytes to main stream", bytes_written);

        for n in 0..bytes_remaining {
            self.outbuffer[n] = self.outbuffer[n + bytes_written];
        }

        self.outbuffer.truncate(bytes_remaining);
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
