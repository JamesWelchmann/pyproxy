use std::fs;
use std::net::TcpListener;
use std::os::unix::net::UnixListener;
use std::process;
use std::rc::Rc;

mod runmaster;
use runmaster::{fatal_io_err, run_forever, Result, Worker};
mod messages;

fn main() -> Result<()> {
    let cfg = runmaster::config::from_env().map(Rc::new)?;

    // Open Tcp Listener
    let main_listener = fatal_io_err(
        "master couldn't bind tcp main listener",
        TcpListener::bind(cfg.bind_addr),
    )?;

    let output_listener = fatal_io_err(
        "master couldn't bind tcp output listener",
        TcpListener::bind(cfg.output_addr),
    )?;

    let pid = unsafe { libc::getpid() };
    let mut sock_addr = cfg.rundir.clone();
    sock_addr.push(&format!("{}", pid));
    fatal_io_err(
        "master failed to create run directory for unix socket",
        fs::create_dir_all(&sock_addr),
    )?;
    sock_addr.push("pyproxy.sock");

    // Open UNIX Socket
    let unix_listener = fatal_io_err(
        "master failed to create unix socket for listening",
        UnixListener::bind(&sock_addr),
    )?;

    // Spawn worker process pool
    let mut workers = Vec::with_capacity(cfg.num_workers);
    for _ in 0..cfg.num_workers {
        let mut child = fatal_io_err(
            "failed to spawn worker process",
            process::Command::new(&cfg.workerbin)
                .arg(&sock_addr)
                .stdout(process::Stdio::piped())
                .stderr(process::Stdio::piped())
                .spawn(),
        )?;

        workers.push(Worker {
            stdout: child.stdout.take().unwrap(),
            stderr: child.stderr.take().unwrap(),
            child,
        });
    }

    run_forever(cfg, main_listener, output_listener, unix_listener, workers)
}
