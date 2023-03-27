use std::collections::HashMap;
use std::os::fd::FromRawFd;
use std::process;
use std::sync::Arc;

use fd_queue::mio::UnixStream;
use mio::net::TcpStream;
use mio::{Events, Interest, Poll, Token};
use serde_json::Value as JValue;

use protocol::RequestMessage;

mod errors;
pub use errors::{Error, Result};
mod clientstream;
pub mod config;
mod pythread;
mod workerstream;

const RO: Interest = Interest::READABLE;
const WORKER_STREAM_TK: Token = Token(0);
const TOKEN_START: usize = 1;

pub fn run_forever(
    cfg: Arc<config::Config>,
    unix_stream: std::os::unix::net::UnixStream,
) -> Result<()> {
    if unix_stream.set_nonblocking(true).is_err() {
        process::exit(1);
    }

    let unix_stream = UnixStream::from_std(unix_stream);
    let mut worker_stream = workerstream::WorkerStream::new(unix_stream);
    let logger = worker_stream.new_logger();
    let (thread_sender, _) = match pythread::start() {
        Ok(s) => s,
        Err(e) => {
            process::exit(1);
        }
    };

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
                println!("got new client stream {:?}", Token(token_io));
                client_streams.insert(Token(token_io), client_stream);
            }

            token_io += 1;
        }

        // Reregister our worker stream RO/RW as needed
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

        // Take any request messages from TcpStreams
        for (tk, client_stream) in client_streams.iter_mut() {
            while let Some(req_msg) = client_stream.next_req_msg() {
                thread_sender.send(req_msg);
            }

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

        // Reregister our TcpStreams as appropriate

        if let Err(_) = poll.poll(&mut events, None) {
            process::exit(1);
        }

        println!("poll wakeup");

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

                continue;
            }

            println!("client stream token = {:?}", ev.token());

            if let Some(client_stream) = client_streams.get_mut(&ev.token()) {
                println!("found client stream");
                if ev.is_readable() {
                    println!("reading from client stream");
                    if client_stream.read(&mut buffer).is_err() {
                        poll.registry().deregister(client_stream).unwrap_or(());
                        to_remove.push(ev.token());
                    }
                    println!("client stream read");
                }

                if ev.is_writable() {
                    // Send response to client
                    if client_stream.write().is_err() {
                        poll.registry().deregister(client_stream).unwrap_or(());
                        to_remove.push(ev.token());
                    } else {
                    }
                }
            }
        }
    }

    Ok(())
}
