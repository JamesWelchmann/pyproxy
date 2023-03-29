use std::cell::RefCell;
use std::collections::VecDeque;
use std::io::{self, Read, Write};
use std::os::fd::RawFd;
use std::rc::Rc;

use fd_queue::mio::UnixStream as FdUnixStream;
use fd_queue::EnqueueFd;
use mio::event::Source;
use mio::net::UnixStream as MioUnixStream;
use mio::{Interest, Registry, Token};
use ndjsonlogger::{error, warn};
use ndjsonloggercore as logger;
use serde_json::Value as JsonValue;

use crate::messages::{self, LogValue};

use super::errors::{self, fatal_io_err};

#[derive(Clone, Debug)]
pub struct WorkerStream {
    inner: Rc<RefCell<Inner>>,
}

pub struct WorkerStreams {
    streams: Vec<(Token, WorkerStream)>,
    last_send: usize,
}

#[derive(Debug)]
struct Inner {
    stream: FdUnixStream,
    outbuffer: Vec<u8>,
    inbuffer: Vec<u8>,

    // Current Mio interest
    interest: Interest,
}

impl WorkerStream {
    pub fn new(stream: MioUnixStream, interest: Interest) -> io::Result<Self> {
        Ok(Self {
            inner: Rc::new(RefCell::new(Inner {
                stream: FdUnixStream::try_from(stream)?,
                outbuffer: Vec::with_capacity(1024),
                inbuffer: Vec::with_capacity(64),
                interest,
            })),
        })
    }

    pub fn dispatch(&mut self, header: [u8; protocol::REQUEST_HEADER_SIZE], fd: RawFd) {
        if self.inner.borrow_mut().send_fd(fd).is_err() {
            error!("master couldn't send fd to worker - queue full");
            return;
        }

        self.inner.borrow_mut().append_buf(&header);
    }

    pub fn has_data(&self) -> bool {
        !self.inner.borrow().outbuffer.is_empty()
    }

    pub fn interest(&self) -> Interest {
        self.inner.borrow().interest
    }

    pub fn set_interest(&self, interest: Interest) {
        self.inner.borrow_mut().interest = interest;
    }

    pub fn write(&self) -> errors::Result<()> {
        self.inner.borrow_mut().write()
    }

    pub fn read(&self, buf: &mut [u8]) -> io::Result<()> {
        self.inner.borrow_mut().read(buf)
    }
}

impl WorkerStreams {
    pub fn new() -> Self {
        Self {
            streams: vec![],
            last_send: 0,
        }
    }

    pub fn add(&mut self, tk: Token, stream: WorkerStream) {
        self.streams.push((tk, stream));
    }

    pub fn dispatch(
        &mut self,
        new_requests: &mut VecDeque<([u8; protocol::REQUEST_HEADER_SIZE], RawFd)>,
    ) {
        if self.streams.is_empty() {
            if !new_requests.is_empty() {
                warn!("no registered workers to dispatch request to");
            }
            return;
        }

        self.last_send += 1;
        self.last_send %= self.streams.len();

        while let Some((header, fd)) = new_requests.pop_front() {
            self.streams[self.last_send].1.dispatch(header, fd);
        }
    }

    pub fn iter_mut(&mut self) -> std::slice::IterMut<(Token, WorkerStream)> {
        self.streams.iter_mut()
    }
}

impl Inner {
    fn send_fd(&mut self, fd: RawFd) -> Result<(), fd_queue::QueueFullError> {
        self.stream.enqueue(&fd)
    }

    fn append_buf(&mut self, header: &[u8; protocol::REQUEST_HEADER_SIZE]) {
        self.outbuffer.extend(header);
    }

    fn write(&mut self) -> errors::Result<()> {
        let bytes_written = fatal_io_err(
            "failed to write to worker stream",
            self.stream.write(&self.outbuffer),
        )?;

        let bytes_remaining = self.outbuffer.len() - bytes_written;
        for n in 0..bytes_remaining {
            self.outbuffer[n] = self.outbuffer[n + bytes_written];
        }
        self.outbuffer.truncate(bytes_remaining);
        Ok(())
    }

    pub fn read(&mut self, buf: &mut [u8]) -> io::Result<()> {
        let bytes_read = self.stream.read(buf)?;
        if bytes_read == 0 {
            Err(io::Error::new(
                io::ErrorKind::ConnectionAborted,
                "worker stream closed",
            ))?;
        }

        self.inbuffer.extend(&buf[..bytes_read]);

        while self.inbuffer.len() >= 5 {
            let msg_type = self.inbuffer[0];
            let msg_len = (u32::from_be_bytes([
                self.inbuffer[1],
                self.inbuffer[2],
                self.inbuffer[3],
                self.inbuffer[4],
            ])) as usize;

            let msg_end = msg_len + 5;

            if self.inbuffer.len() < msg_end {
                break;
            }

            let msg = &self.inbuffer[5..msg_end];

            match msg_type {
                messages::LOG_MESSAGE => {
                    let msg: messages::LogMessage =
                        bincode::deserialize(msg).expect("master couldn't deserialize LogMessage");

                    println!("got log message {:?}", msg);
                }
                messages::PRINT_MESSAGE => {
                    let msg: messages::PrintMessage =
                        bincode::deserialize(msg).expect("master couldn't deserialze PrintMessage");
                }
                _ => {
                    error!("master received unrecognised message type", {
                        "type": u8 = msg_type
                    });
                }
            }

            let bytes_remaining = self.inbuffer.len() - msg_end;
            for n in 0..bytes_remaining {
                self.inbuffer[n] = self.inbuffer[n + msg_end];
            }
            self.inbuffer.truncate(bytes_remaining);
        }

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
        let stream = &mut self.inner.borrow_mut().stream;
        Source::register(stream, registry, token, interests)
    }

    fn reregister(
        &mut self,
        registry: &Registry,
        token: Token,
        interests: Interest,
    ) -> io::Result<()> {
        let stream = &mut self.inner.borrow_mut().stream;
        Source::reregister(stream, registry, token, interests)
    }

    fn deregister(&mut self, registry: &Registry) -> io::Result<()> {
        let stream = &mut self.inner.borrow_mut().stream;
        Source::deregister(stream, registry)
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
