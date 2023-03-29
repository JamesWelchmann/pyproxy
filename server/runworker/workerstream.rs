use std::collections::VecDeque;
use std::io::{self, Read, Write};
use std::os::fd::RawFd;
use std::sync::{Arc, Mutex};

use fd_queue::mio::UnixStream;
use fd_queue::DequeueFd;
use mio::event::Source;
use mio::{Interest, Registry, Token};

use crate::messages::{self, LogLevel, LogMessage, PrintMessage};

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
                inbuffer: Vec::with_capacity(4096),
                outbuffer: Vec::with_capacity(4096),
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
        !self.inner.lock().unwrap().outbuffer.is_empty()
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
    pub fn error(&self, msg: &str, tags: Vec<(&'static str, messages::LogValue)>) {
        self.log(LogLevel::Error, msg, tags);
    }

    pub fn info(&self, msg: &str, tags: Vec<(&'static str, messages::LogValue)>) {
        self.log(LogLevel::Info, msg, tags);
    }

    fn log(&self, level: LogLevel, msg: &str, tags: Vec<(&'static str, messages::LogValue)>) {
        let ts = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Micros, true);
        let msg = bincode::serialize(&LogMessage {
            level,
            ts,
            msg: msg.to_owned(),
            tags: tags.into_iter().map(|(k, v)| (k.to_owned(), v)).collect(),
        })
        .unwrap();

        let msg_len = (msg.len() as u32).to_be_bytes();
        let msg_type = messages::LOG_MESSAGE;
        self.inner.lock().unwrap().new_msg(msg_type, msg_len, &msg);
    }

    pub fn print(&self, message: String) {
        let msg =
            bincode::serialize(&PrintMessage { message }).expect("couldn't serialize PrintMessage");
        let msg_len = (msg.len() as u32).to_be_bytes();
        self.inner
            .lock()
            .unwrap()
            .new_msg(messages::PRINT_MESSAGE, msg_len, &msg);
    }
}

struct Inner {
    stream: UnixStream,
    inbuffer: Vec<u8>,
    outbuffer: Vec<u8>,
    new_msgs: VecDeque<([u8; protocol::REQUEST_HEADER_SIZE], RawFd)>,
}

impl Inner {
    fn new_msg(&mut self, msg_type: u8, msg_len: [u8; 4], msg: &[u8]) {
        self.outbuffer.push(msg_type);
        self.outbuffer.extend(&msg_len);
        self.outbuffer.extend(msg);
    }

    fn read(&mut self, buf: &mut [u8]) -> io::Result<()> {
        let bytes_read = self.stream.read(buf)?;
        if bytes_read == 0 {
            Err(io::Error::new(
                io::ErrorKind::ConnectionAborted,
                "worker stream closed",
            ))?;
        }
        self.inbuffer.extend(&buf[..bytes_read]);

        while self.inbuffer.len() >= protocol::REQUEST_HEADER_SIZE {
            if let Some(fd) = self.stream.dequeue() {
                let mut header = [0; protocol::REQUEST_HEADER_SIZE];
                for (h, b) in header.iter_mut().zip(self.inbuffer.iter()) {
                    *h = *b;
                }

                self.new_msgs.push_back((header, fd));

                let bytes_remaining = self.inbuffer.len() - protocol::REQUEST_HEADER_SIZE;
                for n in 0..bytes_remaining {
                    self.inbuffer[n] = self.inbuffer[n + protocol::REQUEST_HEADER_SIZE];
                }
                self.inbuffer.truncate(bytes_remaining);
            }
            break;
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
