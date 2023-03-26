use std::io::{self, Read, Write};

use mio::event::Source;
use mio::unix::pipe;
use mio::{Interest, Registry, Token};

pub struct PipeFrame {
    buffer: Vec<u8>,
    recv: pipe::Receiver,
}

impl PipeFrame {
    pub fn new(recv: pipe::Receiver) -> Self {
        Self {
            recv,
            buffer: Vec::with_capacity(1024),
        }
    }

    pub fn read(&mut self, buf: &mut [u8]) -> io::Result<()> {
        let bytes_read = self.recv.read(buf)?;
        self.buffer.extend(&buf[..bytes_read]);

        Ok(())
    }

    pub fn write<W: Write>(&mut self, mut dst: W) {
        let mut last_newline = None;
        for (n, c) in self.buffer.iter().enumerate() {
            if *c == b'\n' {
                last_newline = Some(n);
            }
        }

        if let Some(last_newline) = last_newline {
            dst.write_all(&self.buffer[..(last_newline + 1)])
                .expect("couldn't write to process pipe");

            dst.flush().expect("couldn't flush to master process pipe");

            let bytes_remaining = self.buffer.len() - (last_newline + 1);
            for n in 0..bytes_remaining {
                self.buffer[n] = self.buffer[n + (last_newline + 1)];
            }
            self.buffer.truncate(bytes_remaining);
        }
    }
}

impl Source for PipeFrame {
    fn register(
        &mut self,
        registry: &Registry,
        token: Token,
        interests: Interest,
    ) -> io::Result<()> {
        Source::register(&mut self.recv, registry, token, interests)
    }

    fn reregister(
        &mut self,
        registry: &Registry,
        token: Token,
        interests: Interest,
    ) -> io::Result<()> {
        Source::reregister(&mut self.recv, registry, token, interests)
    }

    fn deregister(&mut self, registry: &Registry) -> io::Result<()> {
        Source::deregister(&mut self.recv, registry)
    }
}
