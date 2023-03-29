use std::io::{self, Read, Write};

use mio::event::Source;
use mio::unix::pipe;
use mio::{Interest, Registry, Token};

use protocol::SESSION_ID_LENGTH;

use crate::messages;

#[derive(Debug)]
pub struct PipeFrame {
    buffer: Vec<u8>,
    recv: pipe::Receiver,
    current_session_id: Option<String>,
    last_newline: Option<usize>,
}

impl PipeFrame {
    pub fn new(recv: pipe::Receiver) -> Self {
        Self {
            recv,
            buffer: Vec::with_capacity(1024),
            current_session_id: None,
            last_newline: None,
        }
    }

    pub fn read<'s, W: Write>(
        &'s mut self,
        buf: &mut [u8],
        mut w: W,
    ) -> io::Result<Option<(&'s str, Vec<&'s [u8]>)>> {
        let bytes_read = self.recv.read(buf)?;
        self.buffer.extend(&buf[..bytes_read]);

        let new_req_start_len = messages::NEW_REQUEST_START.as_bytes().len();

        let mut start = 0;
        self.last_newline = None;

        let mut lines = vec![];

        for (n, c) in self.buffer.iter().enumerate() {
            if *c != b'\n' {
                continue;
            }

            let line = &self.buffer[start..n];
            //            println!("reading line {:?}", std::str::from_utf8(line));
            self.last_newline = Some(n);
            start = n + 1;

            // Are we starting a python callable?
            if line.len() == new_req_start_len + (SESSION_ID_LENGTH * 2) {
                if line.starts_with(messages::NEW_REQUEST_START.as_bytes()) {
                    if let Ok(session_id) = std::str::from_utf8(&line[new_req_start_len..]) {
                        println!("new session id = {}", session_id);
                        self.current_session_id = Some(session_id.to_owned());
                        continue;
                    }
                }
            }

            // Are we ending a python callable?
            if line == messages::NEW_REQUEST_END.as_bytes() {
                println!("end session id");
                self.current_session_id = None;
                continue;
            }

            // Save the line if we are in python callable
            if self.current_session_id.is_some() {
                lines.push(line);
            } else {
                // otherwise write it
                w.write_all(line)?;
                w.write_all(b"\n")?;
                w.flush()?;
            }
        }

        Ok(self
            .current_session_id
            .as_ref()
            .map(|s| &s[..])
            .zip(Some(lines)))
    }

    pub fn clear(&mut self) {
        if let Some(last_newline) = self.last_newline {
            let bytes_remaining = self.buffer.len() - last_newline;
            for n in 0..bytes_remaining {
                self.buffer[n] = self.buffer[n + last_newline];
            }

            self.buffer.truncate(bytes_remaining);
        }

        self.last_newline = None;
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
