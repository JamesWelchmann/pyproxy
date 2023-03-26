use std::cell::RefCell;
use std::io::{self, Write};
use std::os::fd::RawFd;
use std::rc::Rc;

use fd_queue::mio::UnixStream as FdUnixStream;
use fd_queue::EnqueueFd;
use mio::event::Source;
use mio::net::UnixStream as MioUnixStream;
use mio::{Interest, Registry, Token};
use ndjsonlogger::{error, warn};

#[derive(Clone)]
pub struct WorkerStream {
    inner: Rc<RefCell<Inner>>,
}

pub struct WorkerStreams {
    streams: Vec<(Token, WorkerStream)>,
    last_send: usize,
}

struct Inner {
    stream: FdUnixStream,
    outbuffer: Vec<u8>,

    // Current Mio interest
    interest: Interest,
}

impl WorkerStream {
    pub fn new(stream: MioUnixStream, interest: Interest) -> io::Result<Self> {
        Ok(Self {
            inner: Rc::new(RefCell::new(Inner {
                stream: FdUnixStream::try_from(stream)?,
                outbuffer: Vec::with_capacity(1024),
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

    pub fn write(&self) -> io::Result<()> {
        self.inner.borrow_mut().write()
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

    pub fn dispatch(&mut self, header: [u8; protocol::REQUEST_HEADER_SIZE], fd: RawFd) {
        if self.streams.is_empty() {
            warn!("no registered workers to dispatch request to");
            return;
        }

        self.last_send += 1;
        self.last_send %= self.streams.len();

        self.streams[self.last_send].1.dispatch(header, fd);
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
