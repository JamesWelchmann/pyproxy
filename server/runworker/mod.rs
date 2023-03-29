use std::collections::HashMap;
use std::os::fd::FromRawFd;
use std::sync::Arc;
use std::time;

use fd_queue::mio::UnixStream;
use mio::net::TcpStream;
use mio::{Events, Interest, Poll, Token};
use ndjsonlogger::info;

use crate::messages::LogValue;

mod errors;
pub use errors::{fatal_io_err, Error, Result};
mod clientstream;
pub mod config;
mod pythread;
mod workerstream;

const RO: Interest = Interest::READABLE;
const WORKER_STREAM_TK: Token = Token(0);
const TOKEN_START: usize = 1;
const POLL_TIME: time::Duration = time::Duration::from_millis(100);

pub fn run_forever(
    cfg: Arc<config::Config>,
    unix_stream: std::os::unix::net::UnixStream,
) -> Result<()> {
    info!("worker started");

    fatal_io_err(
        "worker couldn't set unix stream to non-blocking",
        unix_stream.set_nonblocking(true),
    )?;

    let unix_stream = UnixStream::from_std(unix_stream);
    let mut worker_stream = workerstream::WorkerStream::new(unix_stream);
    let logger = worker_stream.new_logger();
    let (thread_sender, _) = pythread::start(logger.clone())?;

    let mut poll = fatal_io_err("worker couldn't create mio poll instance", Poll::new())?;

    let mut ws_interest = RO;

    fatal_io_err(
        "worker couldn't register unix stream with mio poll",
        poll.registry()
            .register(&mut worker_stream, WORKER_STREAM_TK, ws_interest),
    )?;

    let mut events = Events::with_capacity(1024);
    let mut buffer = vec![0; 4096];
    let mut token_io = TOKEN_START;
    let mut client_streams = HashMap::new();
    let mut to_remove = vec![];

    loop {
        // Take any new streams
        while let Some((header, fd)) = worker_stream.next_msg() {
            let stream = unsafe { std::net::TcpStream::from_raw_fd(fd) };
            stream.set_nonblocking(true);
            let stream = TcpStream::from_std(stream);
            let mut client_stream = match clientstream::ClientStream::new(&cfg, header, stream, RO)
            {
                Ok(cs) => cs,
                Err(_) => {
                    // Just drop bad client
                    continue;
                }
            };
            if poll
                .registry()
                .register(&mut client_stream, Token(token_io), RO)
                .is_ok()
            {
                logger.info(
                    "new client mainstream started",
                    vec![(
                        "session_id",
                        LogValue::String(client_stream.session_id().to_owned()),
                    )],
                );
                client_streams.insert(Token(token_io), client_stream);
            }

            token_io += 1;
        }

        // Reregister our worker stream RO/RW as needed
        if ws_interest == RO && worker_stream.has_data() {
            ws_interest = Interest::READABLE | Interest::WRITABLE;
            fatal_io_err(
                "worker couldn't register unix stream RO",
                poll.registry()
                    .reregister(&mut worker_stream, WORKER_STREAM_TK, ws_interest),
            )?;
        } else if ws_interest.is_writable() && !worker_stream.has_data() {
            ws_interest = RO;
            fatal_io_err(
                "worker couldn't register unix stream RW",
                poll.registry()
                    .reregister(&mut worker_stream, WORKER_STREAM_TK, ws_interest),
            )?;
        }

        // Take any request messages from TcpStreams
        for (tk, client_stream) in client_streams.iter_mut() {
            while let Some(req_msg) = client_stream.next_req_msg() {
                logger.info(
                    "queueing new pyproxy atom processing",
                    vec![
                        (
                            "session_id",
                            LogValue::String(client_stream.session_id().to_owned()),
                        ),
                        (
                            "future_id",
                            LogValue::String(req_msg.future_id().unwrap_or("0000").to_owned()),
                        ),
                    ],
                );
                thread_sender.send((client_stream.session_id().to_owned(), req_msg));
            }

            // Reregister client stream RO or RW
            if client_stream.interest() == RO && client_stream.has_out_data() {
                let i = Interest::READABLE | Interest::WRITABLE;
                client_stream.set_interest(i);
                if poll.registry().reregister(client_stream, *tk, i).is_err() {
                    to_remove.push(*tk);
                }
            } else if client_stream.interest().is_writable() && !client_stream.has_out_data() {
                client_stream.set_interest(RO);
                if poll.registry().reregister(client_stream, *tk, RO).is_err() {
                    to_remove.push(*tk);
                }
            }
        }

        fatal_io_err(
            "worker couldn't call mio poll",
            poll.poll(&mut events, None),
        )?;

        for ev in &events {
            if ev.token() == WORKER_STREAM_TK {
                if ev.is_readable() {
                    // Read the message
                    fatal_io_err(
                        "worker failed to read worker stream",
                        worker_stream.read(&mut buffer),
                    )?;
                }

                if ev.is_writable() {
                    fatal_io_err(
                        "worker failed to write on worker stream",
                        worker_stream.write(),
                    )?;
                }

                continue;
            }

            println!("reading client stream {:?}", ev);

            if let Some(client_stream) = client_streams.get_mut(&ev.token()) {
                if ev.is_readable() {
                    if client_stream.read(&mut buffer).is_err() {
                        poll.registry().deregister(client_stream).unwrap_or(());
                        to_remove.push(ev.token());
                    }
                }

                if ev.is_writable() {
                    // Send response to client
                    if client_stream.write().is_err() {
                        poll.registry().deregister(client_stream).unwrap_or(());
                        to_remove.push(ev.token());
                    } else {
                        println!("written to client stream");
                    }
                }
            }
        }
    }
}
