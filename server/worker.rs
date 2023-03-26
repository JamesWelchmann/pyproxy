use std::env;
use std::os::unix::net::UnixStream;
use std::process;
use std::sync::Arc;

use ndjsonlogger::error;

mod runworker;
use runworker::{config, run_forever, Error, Result};
mod messages;

fn main() -> Result<()> {
    let cfg = config::from_env().map(Arc::new)?;
    let mut sock_path = None;

    for (n, arg) in env::args().enumerate() {
        match n {
            0 => {}
            1 => {
                sock_path = Some(arg.clone());
            }
            _ => {
                return Err(Error::InvalidArgs);
            }
        }
    }

    let sock_path = sock_path.ok_or(Error::InvalidArgs)?;

    // Connect to the unix socket
    let stream = match UnixStream::connect(&sock_path) {
        Ok(stream) => stream,
        Err(io_err) => {
            error!("worker failed to connect master process", {
                error = &format!("{}", io_err)
            });
            process::exit(1);
        }
    };

    run_forever(cfg, stream)
}
