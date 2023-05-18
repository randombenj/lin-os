use std::path::Path;

use log::debug;
use nix::mount::{mount, MsFlags};

/// Mounts the proc filesystem to '/proc'.
///
/// # Panics
///
/// Panics if mounting fails.
pub fn mount_proc() {
    mount(
        None::<&str>,
        Path::new("/proc"),
        Some("proc"),
        MsFlags::MS_RDONLY,
        None::<&str>,
    )
    .unwrap_or_else(|err| panic!("Mounting /proc failed!!\n{:?}", err));
}

/// Initializes the filesystem:
///
/// - Mount the root disk is mounted to '/'
///
/// # Arguments
///
/// * `root_disk` - The path to the root disk
///                (e.g. '/dev/sda' or '/dev/vda')
///
/// # Panics
///
/// Panics if mounting any of the filesystems fails.
pub fn mount_filesystem(root_disk: &str) {
    debug!("Mounting '/ -> {}'", root_disk);
    mount(
        Some(root_disk),
        Path::new("/"),
        Some(""),
        MsFlags::MS_REMOUNT,
        None::<&str>,
    )
    .unwrap_or_else(|err| panic!("Mounting '/ -> {}' failed!!\n{:?}", root_disk, err));
}
