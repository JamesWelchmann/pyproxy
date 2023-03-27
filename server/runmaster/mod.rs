use std::collections::HashMap;
use std::io;
use std::process;
use std::rc::Rc;

use mio::net::{TcpListener, UnixListener};
use mio::{unix::pipe, Events, Interest, Poll, Token};
use ndjsonlogger::error;

mod errors;
pub use errors::{fatal_io_err, Error, Result};
pub mod config;
use config::Config;
mod clientstream;
mod pipeframe;
mod workerstream;

pub struct Worker {
    pub child: process::Child,
    pub stdout: process::ChildStdout,
    pub stderr: process::ChildStderr,
}

const MAIN_LISTENER_TK: Token = Token(0);
const OUTPUT_LISTENER_TK: Token = Token(1);
const UNIX_LISTENER_TK: Token = Token(2);
const TOKEN_START: usize = 3;
const RO: Interest = Interest::READABLE;

enum IoAction {
    MainListener(TcpListener),
    OutputListener(TcpListener),
    ClientStream(clientstream::ClientStream),
    UnixListener(UnixListener),
    Stderr(pipeframe::PipeFrame),
    Stdout(pipeframe::PipeFrame),
    WorkerStream(workerstream::WorkerStream),
}

pub fn run_forever(
    cfg: Rc<Config>,
    main_listener: std::net::TcpListener,
    output_listener: std::net::TcpListener,
    unix_listener: std::os::unix::net::UnixListener,
    workers: Vec<Worker>,
) -> Result<()> {
    fatal_io_err(
        "master tcp listener couldn't be set non blocking",
        main_listener.set_nonblocking(true),
    )?;

    fatal_io_err(
        "output tcp listener couldn't be set non blocking",
        output_listener.set_nonblocking(true),
    )?;

    fatal_io_err(
        "master unix listener couldn't be set non blocking",
        unix_listener.set_nonblocking(true),
    )?;

    let mut main_listener = TcpListener::from_std(main_listener);
    let mut output_listener = TcpListener::from_std(output_listener);
    let mut unix_listener = UnixListener::from_std(unix_listener);

    let mut poll = fatal_io_err("master failed to create mio poll instance", Poll::new())?;

    let mut io_token = TOKEN_START;
    let mut io_actions = HashMap::new();

    // Register tcp listeners
    fatal_io_err(
        "master failed to register main tcp listener for reading",
        poll.registry()
            .register(&mut main_listener, MAIN_LISTENER_TK, RO),
    )?;
    io_actions.insert(MAIN_LISTENER_TK, IoAction::MainListener(main_listener));

    fatal_io_err(
        "master failed to register output tcp listener for reading",
        poll.registry()
            .register(&mut output_listener, OUTPUT_LISTENER_TK, RO),
    )?;
    io_actions.insert(
        OUTPUT_LISTENER_TK,
        IoAction::OutputListener(output_listener),
    );

    // Register unix listener
    fatal_io_err(
        "master failed to register unix listener for reading",
        poll.registry()
            .register(&mut unix_listener, UNIX_LISTENER_TK, RO),
    )?;
    io_actions.insert(UNIX_LISTENER_TK, IoAction::UnixListener(unix_listener));

    let mut events = Events::with_capacity(1024);

    // Register stdout/stderr of workers
    for w in workers {
        let mut stdout = pipe::Receiver::from(w.stdout);
        // Stdout
        fatal_io_err(
            "master couldn't set worker stdout to non-blocking",
            stdout.set_nonblocking(true),
        )?;

        fatal_io_err(
            "master couldn't register worker stdout for reading",
            poll.registry().register(&mut stdout, Token(io_token), RO),
        )?;

        io_actions.insert(
            Token(io_token),
            IoAction::Stdout(pipeframe::PipeFrame::new(stdout)),
        );

        io_token += 1;

        // Stderr
        let mut stderr = pipe::Receiver::from(w.stderr);
        fatal_io_err(
            "master couldn't set worker stderr to non-blocking",
            stderr.set_nonblocking(true),
        )?;

        fatal_io_err(
            "master couldn't register worker stderr for reading",
            poll.registry().register(&mut stderr, Token(io_token), RO),
        )?;
        io_actions.insert(
            Token(io_token),
            IoAction::Stderr(pipeframe::PipeFrame::new(stderr)),
        );

        io_token += 1;
    }

    let mut buffer = vec![0; 4096];
    let mut to_remove = Vec::with_capacity(16);
    let mut worker_streams = workerstream::WorkerStreams::new();
    let mut new_requests = Vec::with_capacity(64);

    loop {
        for tk in to_remove.drain(..) {
            io_actions.remove(&tk);
        }

        for (header, fd) in new_requests.drain(..) {
            // Dispatch this client session to a worker
            worker_streams.dispatch(header, fd);
        }

        // Do we need to write to our worker streams?
        for (tk, worker_stream) in worker_streams.iter_mut() {
            if worker_stream.has_data() && worker_stream.interest() == RO {
                worker_stream.set_interest(Interest::READABLE | Interest::WRITABLE);
                fatal_io_err(
                    "master couldn't re-register worker stream RW",
                    poll.registry()
                        .reregister(worker_stream, *tk, worker_stream.interest()),
                )?;
            }

            if !worker_stream.has_data() && worker_stream.interest().is_writable() {
                worker_stream.set_interest(RO);
                fatal_io_err(
                    "master couldn't re-register worker stream RO",
                    poll.registry()
                        .reregister(worker_stream, *tk, worker_stream.interest()),
                )?;
            }
        }

        fatal_io_err(
            "master failed to poll mio for events",
            poll.poll(&mut events, None),
        )?;

        for ev in &events {
            println!("event = {:?}", ev.token());
            match io_actions.get_mut(&ev.token()) {
                None => {
                    error!("master didn't find token in io_actions map");
                }
                Some(IoAction::MainListener(main_listener)) => match main_listener.accept() {
                    Ok((stream, _)) => {
                        let mut client_stream = clientstream::ClientStream::new(stream);
                        if poll
                            .registry()
                            .register(&mut client_stream, Token(io_token), RO)
                            .is_ok()
                        {
                            io_actions
                                .insert(Token(io_token), IoAction::ClientStream(client_stream));
                        }

                        // Ignore errors

                        io_token += 1;
                    }
                    Err(_) => {
                        // Ignore the error - just drop the stream
                    }
                },
                Some(IoAction::OutputListener(output_listener)) => match output_listener.accept() {
                    Ok((mut stream, _)) => {
                        if poll
                            .registry()
                            .register(&mut stream, Token(io_token), RO)
                            .is_ok()
                        {
                            // TODO: Register output stream
                        }

                        // Ignore errors

                        io_token += 1;
                    }
                    Err(_) => {
                        // Ignore the error - just drop the stream
                    }
                },
                Some(IoAction::ClientStream(client_stream)) => {
                    match client_stream.read() {
                        clientstream::ReadResult::Continue => {
                            // Keep waiting for message header
                        }
                        clientstream::ReadResult::Closed => {
                            // TcpStream closed - remove it
                            poll.registry().deregister(client_stream).unwrap_or(());
                            to_remove.push(ev.token());
                        }
                        clientstream::ReadResult::Done => {
                            poll.registry().deregister(client_stream).unwrap_or(());

                            // NOTE: We don't remove it - leave it hanging around
                            // in the io_actions HashMap
                            let new_req = (client_stream.header(), client_stream.raw_fd());
                            new_requests.push(new_req);
                        }
                        clientstream::ReadResult::Error(_) => {
                            // Error reading TcpStream - ignore it
                            poll.registry().deregister(client_stream).unwrap_or(());
                            to_remove.push(ev.token());
                        }
                    }
                }
                Some(IoAction::UnixListener(unix_listener)) => match unix_listener.accept() {
                    Err(io_err) => {
                        error!("master unix listener received error on worker connect", {
                            error = &format!("{}", io_err)
                        });
                    }
                    Ok((mut stream, _)) => {
                        fatal_io_err(
                            "master failed to register worker stream for reading",
                            poll.registry().register(&mut stream, Token(io_token), RO),
                        )?;
                        let worker_stream = fatal_io_err(
                            "master couldn't create worker stream instance",
                            workerstream::WorkerStream::new(stream, RO),
                        )?;
                        io_actions.insert(
                            Token(io_token),
                            IoAction::WorkerStream(worker_stream.clone()),
                        );
                        worker_streams.add(Token(io_token), worker_stream);
                        io_token += 1;
                    }
                },
                Some(IoAction::WorkerStream(worker_stream)) => {
                    if ev.is_readable() {
                        // TODO
                    }

                    if ev.is_writable() {
                        if let Err(err) = worker_stream.write() {
                            error!("failed to write to worker stream", {
                                error = &format!("{}", err)
                            });
                        }
                    }
                }
                Some(IoAction::Stdout(pipeframe)) => match pipeframe.read(&mut buffer) {
                    Err(err) => {
                        error!("broken pipe with worker stdout", {
                            error = &format!("{}", err)
                        });

                        poll.registry().deregister(pipeframe).unwrap_or(());
                        to_remove.push(ev.token());
                    }
                    Ok(_) => {
                        pipeframe.write(io::stdout());
                    }
                },
                Some(IoAction::Stderr(pipeframe)) => match pipeframe.read(&mut buffer) {
                    Err(err) => {
                        error!("broken pipe with worker stderr", {
                            error = &format!("{}", err)
                        });

                        poll.registry().deregister(pipeframe).unwrap_or(());
                        to_remove.push(ev.token());
                    }
                    Ok(_) => {
                        pipeframe.write(io::stderr());
                    }
                },
            }
        }
    }
}
