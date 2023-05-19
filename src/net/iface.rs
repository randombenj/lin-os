use std::{mem, net::IpAddr, ptr};

use libc::{self, ifreq};
use nix::{ioctl_write_ptr_bad, sys::socket, unistd::close};

use super::service::NetworkError;

ioctl_write_ptr_bad!(siocsifflags, libc::SIOCSIFFLAGS, libc::ifreq);
ioctl_write_ptr_bad!(siocsifaddr, libc::SIOCSIFADDR, libc::ifreq);

pub struct ConfigSocket {
    pub fd: i32,
    pub iface: String,
}

impl Drop for ConfigSocket {
    fn drop(&mut self) {
        close(self.fd).unwrap();
    }
}

impl ConfigSocket {
    pub(crate) fn new(iface: String) -> Result<ConfigSocket, NetworkError> {
        if iface.len() >= libc::IFNAMSIZ {
            return Err(NetworkError {
                message: "interface name too long".to_string(),
                err: nix::errno::Errno::ENAMETOOLONG,
            });
        }

        let fd = match socket::socket(
            socket::AddressFamily::Inet,
            socket::SockType::Datagram,
            socket::SockFlag::empty(),
            None,
        ) {
            Ok(fd) => fd,
            Err(err) => {
                return Err(NetworkError {
                    message: "failed to create config socket".to_string(),
                    err: err,
                });
            }
        };

        Ok(ConfigSocket { fd, iface })
    }

    unsafe fn request(&self) -> ifreq {
        let mut req: ifreq = mem::zeroed();
        ptr::copy_nonoverlapping(
            self.iface.as_ptr() as *const libc::c_char,
            req.ifr_name.as_mut_ptr(),
            self.iface.len(),
        );

        req
    }

    pub(crate) fn enable(&self, value: bool) -> Result<(), NetworkError> {
        unsafe {
            let mut req = self.request();

            if let Err(err) = siocsifflags(self.fd, &mut req) {
                return Err(NetworkError {
                    message: "failed to set interface flags".to_string(),
                    err: err,
                });
            }

            if value {
                req.ifr_ifru.ifru_flags |= libc::IFF_UP as i16 | libc::IFF_UP as i16;
            } else {
                req.ifr_ifru.ifru_flags &= !libc::IFF_UP as i16;
            }

            if let Err(err) = siocsifflags(self.fd, &req) {
                return Err(NetworkError {
                    message: "failed to set interface flags".to_string(),
                    err: err,
                });
            }
        }

        Ok(())
    }

    pub(crate) fn set_ip(&self, addr: IpAddr) -> Result<(), NetworkError> {
        let ip = match addr {
            IpAddr::V4(ip) => ip,
            IpAddr::V6(_) => {
                return Err(NetworkError {
                    message: "IPv6 not supported".to_string(),
                    err: nix::errno::Errno::EAFNOSUPPORT,
                })
            }
        };

        unsafe {
            let mut req = self.request();

            req.ifr_ifru.ifru_addr.sa_family = libc::AF_INET as u16;
            ip.octets().iter().enumerate().for_each(|(i, octet)| {
                req.ifr_ifru.ifru_addr.sa_data[i] = *octet as i8;
            });
        }

        Ok(())
    }
}
