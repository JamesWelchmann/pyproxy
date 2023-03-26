use std::collections::HashMap;
use std::os::fd::FromRawFd;
use std::process;
use std::sync::Arc;

use fd_queue::mio::UnixStream;
use mio::net::TcpStream;
use mio::{Events, Interest, Poll, Token};
use serde_json::Value as JValue;

mod errors;
pub use errors::{Error, Result};
pub mod config;
use config::Config;
mod clientstream;
mod workerstream;

const RO: Interest = Interest::READABLE;
const WORKER_STREAM_TK: Token = Token(0);
const TOKEN_START: usize = 1;

pub fn run_forever(
    cfg: Arc<config::Config>,
    unix_stream: std::os::unix::net::UnixStream,
) -> Result<()> {
    unix_stream.set_nonblocking(true);
    let mut unix_stream = UnixStream::from_std(unix_stream);
    let mut worker_stream = workerstream::WorkerStream::new(unix_stream);
    let logger = worker_stream.new_logger();

    let mut poll = match Poll::new() {
        Ok(poll) => poll,
        Err(_) => {
            process::exit(1);
        }
    };

    let mut ws_interest = RO;

    if poll
        .registry()
        .register(&mut worker_stream, WORKER_STREAM_TK, ws_interest)
        .is_err()
    {
        process::exit(1);
    }

    let mut events = Events::with_capacity(1024);
    let mut buffer = vec![0; 4096];
    let mut token_io = TOKEN_START;
    let mut client_streams = HashMap::new();
    let mut to_remove = vec![];

    loop {
        // Reregister our stream RO/RW as needed
        if ws_interest == RO && worker_stream.has_data() {
            ws_interest = Interest::READABLE | Interest::WRITABLE;
            if poll
                .registry()
                .reregister(&mut worker_stream, WORKER_STREAM_TK, ws_interest)
                .is_err()
            {
                process::exit(1);
            }
        } else if ws_interest.is_writable() && !worker_stream.has_data() {
            ws_interest = RO;
            if poll
                .registry()
                .reregister(&mut worker_stream, WORKER_STREAM_TK, ws_interest)
                .is_err()
            {
                process::exit(1);
            }
        }

        // Take any new messages
        while let Some((header, fd)) = worker_stream.next_msg() {
            let stream = unsafe { TcpStream::from_raw_fd(fd) };
            let interest = Interest::WRITABLE;
            let mut client_stream =
                match clientstream::ClientStream::new(&cfg, header, stream, interest) {
                    Ok(cs) => cs,
                    Err(_) => {
                        // Just drop bad client
                        continue;
                    }
                };
            if poll
                .registry()
                .register(&mut client_stream, Token(token_io), interest)
                .is_ok()
            {
                client_streams.insert(Token(token_io), client_stream);
            }

            token_io += 1;
        }

        if let Err(_) = poll.poll(&mut events, None) {
            process::exit(1);
        }

        for ev in &events {
            if ev.token() == WORKER_STREAM_TK {
                if ev.is_readable() {
                    // Read the message
                    worker_stream.read(&mut buffer);
                }

                if ev.is_writable() {
                    // Write to unix socket
                    worker_stream.write();
                }
            }

            println!("got client stream");

            if let Some(client_stream) = client_streams.get_mut(&ev.token()) {
                if ev.is_readable() {}

                if ev.is_writable() {
                    println!("writing to client stream");
                    // Send response to client
                    if client_stream.write().is_err() {
                        poll.registry().deregister(client_stream).unwrap_or(());
                        to_remove.push(ev.token());
                    } else {
                        println!("write ok");
                    }
                }
            }
        }
    }

    Ok(())
}
