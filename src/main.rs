//! linµos is a simple linux init system.
//!
//! It is designed to be used as a PID 1 init system
//! for single binary linux distributions.
//!
//! The main goal is to avoid the complexity of maintaining
//! and patching a full blown linux distribution.

pub mod fs;
pub mod net;

use std::{
    env,
    fs::File,
    io,
    process::{Command, Stdio},
};

use env_logger;
use log::{debug, info};

/// Represents arguments parsed from
/// the kernel command line.
#[derive(Debug)]
struct Cmdline {
    quiet: bool,
    root: String,
}

/// Parses the kernel command line by reading `/proc/cmdline`.
///
/// The proc filesystem must be mounted before running
/// this function.
fn parse_cmdline() -> Cmdline {
    let args = match std::fs::read_to_string("/proc/cmdline") {
        Ok(contents) => contents,
        Err(err) => panic!("Could not read /proc/cmdline: {:?}", err),
    };
    let args = args.trim().split_whitespace().collect::<Vec<&str>>();

    let quiet = match args.iter().find(|arg| arg.starts_with("quiet")) {
        Some(_) => true,
        None => false,
    };

    let root = match args.iter().find(|arg| arg.starts_with("root=")) {
        Some(arg) => arg,
        None => panic!("No root device specified"),
    };
    let root_device = root.trim_start_matches("root=");

    Cmdline {
        quiet,
        root: root_device.to_string(),
    }
}

fn exec(program: &str, args: &[&str], wait: bool, log: bool) -> io::Result<()> {
    let mut cmd = Command::new(program);
    cmd.args(args);

    if log {
        let file = File::create("/var/log/syslog").unwrap();
        let stdio = Stdio::from(file);

        let file_err = File::create("/var/log/syslog").unwrap();
        let sterr = Stdio::from(file_err);

        cmd.stdout(stdio);
        cmd.stderr(sterr);
    }

    let mut cmd = cmd.spawn()?;

    if wait {
        let status = cmd.wait()?;
        if !status.success() {
            eprintln!("{} ran, but indicated failure: {:?}", program, status);
        }
    }

    Ok(())
}

fn main() {
    env::set_var("RUST_BACKTRACE", "1");
    // -- parse kernel command line arguments
    if let Err(err) = fs::mount::proc() {
        panic!("[panic] failed mounting filesystem: {}", err)
    }
    let cmdline = parse_cmdline();

    // -- set up logging
    let env = env_logger::Env::new()
        .filter_or("LOG", if cmdline.quiet { "warn" } else { "trace" })
        .write_style("LOG_STYLE");
    env_logger::init_from_env(env);

    // -- system startup
    info!(" => starting linµos");
    debug!("{:?}", cmdline);

    if let Err(err) = fs::mountfs(&cmdline.root) {
        panic!("[panic] failed mounting filesystem: {}", err)
    }

    if let Err(err) = net::configure_network() {
        panic!("[panic] failed configuring network: {}", err)
    }

    std::env::set_current_dir("/").unwrap();
    exec("/busybox", &["--install"], false, false).unwrap();
    // exec("/k3s", &["server"], false, true).unwrap();
    exec("/busybox", &["sh"], true, false).unwrap();

    panic!("[panic] init tried to return!");
}
