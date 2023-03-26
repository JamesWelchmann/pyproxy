use std::collections::{HashMap, VecDeque};
use std::io::{self, Read, Write};
use std::os::fd::RawFd;
use std::sync::{Arc, Mutex};

use fd_queue::mio::UnixStream;
use fd_queue::DequeueFd;
use mio::event::Source;
use mio::{Interest, Registry, Token};
use ndjsonloggercore::{Atom as LogAtom, Entry as LogEntry, Value as LogValue};
use serde_json::Value as JValue;

use crate::messages::{self, LogLevel, LogMessage};

#[derive(Clone)]
pub struct WorkerStream {
    inner: Arc<Mutex<Inner>>,
}

#[derive(Clone)]
pub struct Logger {
    inner: Arc<Mutex<Inner>>,
}

impl WorkerStream {
    pub fn new(stream: UnixStream) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                stream,
                buffer: Vec::with_capacity(4096),
                new_msgs: VecDeque::with_capacity(64),
            })),
        }
    }

    pub fn new_logger(&self) -> Logger {
        Logger {
            inner: self.inner.clone(),
        }
    }

    pub fn has_data(&self) -> bool {
        !self.inner.lock().unwrap().buffer.is_empty()
    }

    pub fn read(&self, buffer: &mut [u8]) -> io::Result<()> {
        self.inner.lock().unwrap().read(buffer)
    }

    pub fn write(&self) -> io::Result<()> {
        self.inner.lock().unwrap().write()
    }

    pub fn next_msg(&self) -> Option<([u8; protocol::REQUEST_HEADER_SIZE], RawFd)> {
        self.inner.lock().unwrap().new_msgs.pop_front()
    }
}

impl Logger {
    pub fn error(&self, msg: &str, tags: Vec<(&'static str, JValue)>) {
        self.log(LogLevel::Error, msg, tags);
    }

    pub fn info(&self, msg: &str, tags: Vec<(&'static str, JValue)>) {
        self.log(LogLevel::Info, msg, tags);
    }

    fn log(&self, level: LogLevel, msg: &str, log_tags: Vec<(&'static str, JValue)>) {
        let mut tags = HashMap::with_capacity(log_tags.len());
        for (k, v) in log_tags.into_iter() {
            tags.insert(k.to_owned(), v);
        }

        let ts = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Micros, true);
        let msg = bincode::serialize(&LogMessage {
            level,
            ts,
            msg: msg.to_owned(),
            tags,
        })
        .unwrap();

        let msg_len = (msg.len() as u32).to_be_bytes();
        let msg_type = messages::LOG_MESSAGE;
        self.inner.lock().unwrap().new_msg(msg_type, msg_len, &msg);
    }
}

struct Inner {
    stream: UnixStream,
    buffer: Vec<u8>,
    new_msgs: VecDeque<([u8; protocol::REQUEST_HEADER_SIZE], RawFd)>,
}

impl Inner {
    fn new_msg(&mut self, msg_type: u8, msg_len: [u8; 4], msg: &[u8]) {
        self.buffer.push(msg_type);
        self.buffer.extend(&msg_len);
        self.buffer.extend(msg);
    }

    fn read(&mut self, buf: &mut [u8]) -> io::Result<()> {
        let bytes_read = self.stream.read(buf)?;
        self.buffer.extend(&buf[..bytes_read]);

        while self.buffer.len() >= protocol::REQUEST_HEADER_SIZE {
            if let Some(fd) = self.stream.dequeue() {
                let mut header = [0; protocol::REQUEST_HEADER_SIZE];
                for (h, b) in header.iter_mut().zip(self.buffer.iter()) {
                    *h = *b;
                }

                self.new_msgs.push_back((header, fd));

                let bytes_remaining = self.buffer.len() - protocol::REQUEST_HEADER_SIZE;
                for n in 0..bytes_remaining {
                    self.buffer[n] = self.buffer[n + 12];
                }
                self.buffer.truncate(bytes_remaining);
            }
        }

        Ok(())
    }

    fn write(&mut self) -> io::Result<()> {
        let bytes_written = self.stream.write(&self.buffer)?;
        let bytes_remaining = self.buffer.len() - bytes_written;

        for n in 0..bytes_remaining {
            self.buffer[n] = self.buffer[n + bytes_written];
        }
        self.buffer.truncate(bytes_remaining);
        Ok(())
    }
}

impl Source for WorkerStream {
    fn register(
        &mut self,
        registry: &Registry,
        token: Token,
        interests: Interest,
    ) -> io::Result<()> {
        let mut stream = self.inner.lock().unwrap();
        Source::register(&mut (*stream), registry, token, interests)
    }

    fn reregister(
        &mut self,
        registry: &Registry,
        token: Token,
        interests: Interest,
    ) -> io::Result<()> {
        let mut stream = self.inner.lock().unwrap();
        Source::reregister(&mut (*stream), registry, token, interests)
    }

    fn deregister(&mut self, registry: &Registry) -> io::Result<()> {
        let mut stream = self.inner.lock().unwrap();
        Source::deregister(&mut (*stream), registry)
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
