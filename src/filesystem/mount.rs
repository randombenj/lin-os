use std::path::Path;

use nix::mount::{MsFlags, mount};

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
    .unwrap_or_else(|err| panic!("[panic]: mounting /proc failed!!\n{:?}", err));
}

/// Initializes the filesystem:
///
/// - the root disk is mounted to '/'
/// - devtmpfs is mounted to '/dev'
/// - sysfs is mounted to '/sys'
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
    mount(
        Some(root_disk),
        Path::new("/"),
        Some(""),
        MsFlags::MS_REMOUNT,
        None::<&str>,
    )
    .unwrap_or_else(|err| panic!("[panic]: mounting '/ -> {}' failed!!\n{:?}", root_disk, err));

    mount(
        None::<&str>,
        Path::new("/sys"),
        Some("sysfs"),
        MsFlags::MS_RDONLY,
        None::<&str>,
    )
    .unwrap_or_else(|err| panic!("[panic]: mounting /sys failed!\n{:?}", err));

    mount(
        None::<&str>,
        Path::new("/dev"),
        Some("devtmpfs"),
        MsFlags::MS_RDONLY,
        None::<&str>,
    )
    .unwrap_or_else(|err| panic!("[panic]: mounting /dev failed!\n{:?}", err));
}
