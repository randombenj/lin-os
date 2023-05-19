use core::fmt;
use std::path::Path;

use nix::mount::{mount, MsFlags};

#[derive(Debug, Clone)]
pub struct MountError {
    pub mountpoint: String,
    pub err: nix::Error,
}

impl fmt::Display for MountError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "mount error: {} -> {}", self.mountpoint, self.err)
    }
}

/// Mounts the proc filesystem at `/proc`.
pub fn proc() -> Result<(), MountError> {
    if let Err(err) = mount(
        None::<&str>,
        Path::new("/proc"),
        Some("proc"),
        MsFlags::empty(),
        None::<&str>,
    ) {
        return Err(MountError {
            mountpoint: "/proc".to_string(),
            err: err,
        });
    }

    Ok(())
}

/// Sets up the required filesystems for the system to boot.
/// This includes mounting /tmp, /proc, /dev, / and /sys.
///
/// # Arguments
///
/// * `root_disk` - The path to the root disk
///                (e.g. '/dev/sda' or '/dev/vda')
///
/// # Panics
///
/// Panics if mounting any of the filesystems fails.
pub fn mountfs(root_disk: &str) -> Result<(), MountError> {
    if let Err(err) = mount(
        Some("tmpfs"),
        Path::new("/tmp"),
        Some("tmpfs"),
        MsFlags::MS_NOSUID | MsFlags::MS_NODEV | MsFlags::MS_RELATIME,
        None::<&str>,
    ) {
        return Err(MountError {
            mountpoint: "/tmp".to_string(),
            err: err,
        });
    }

    if let Err(err) = mount(
        None::<&str>,
        Path::new("/proc"),
        Some("proc"),
        MsFlags::empty(),
        None::<&str>,
    ) {
        return Err(MountError {
            mountpoint: "/proc".to_string(),
            err: err,
        });
    }

    if let Err(err) = mount(
        Some("devtmpfs"),
        Path::new("/dev"),
        Some("devtmpfs"),
        MsFlags::empty(),
        None::<&str>,
    ) {
        if err != nix::errno::Errno::EBUSY {
            return Err(MountError {
                mountpoint: "/dev".to_string(),
                err: err,
            });
        } // otherwise /dev is already mounted
    }

    if let Err(err) = mount(
        Some(root_disk),
        Path::new("/"),
        Some(""),
        MsFlags::MS_REMOUNT,
        None::<&str>,
    ) {
        return Err(MountError {
            mountpoint: format!("/ -> {}", root_disk),
            err: err,
        });
    }

    if let Err(err) = mount(
        Some("sysfs"),
        Path::new("/sys"),
        Some("sysfs"),
        MsFlags::MS_RDONLY,
        None::<&str>,
    ) {
        return Err(MountError {
            mountpoint: "/sys".to_string(),
            err: err,
        });
    }

    Ok(())
}
