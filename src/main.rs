//! linµos is a simple linux init system.
//!
//! It is designed to be used as a PID 1 init system
//! for single binary linux distributions.
//!
//! The main goal is to avoid the complexity of maintaining
//! and patching a full blown linux distribution.


pub mod filesystem;

use std::fs;

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
    let args = match fs::read_to_string("/proc/cmdline") {
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

fn main() {
    // -- parse kernel command line arguments
    filesystem::mount::mount_proc();
    let cmdline = parse_cmdline();

    // -- set up logging
    let env = env_logger::Env::new()
        .filter_or("LOG", if cmdline.quiet { "warn" } else { "debug" })
        .write_style("LOG_STYLE");
    env_logger::init_from_env(env);

    info!(" => starting linµos");
    debug!("{:?}", cmdline);

    filesystem::mount::mount_filesystem(&cmdline.root);

    panic!("[panic] init tried to return!");
}
