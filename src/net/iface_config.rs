use std::{ffi::CString, fs, mem, net::IpAddr, ptr};

use libc;
use nix::{ioctl_write_ptr_bad, sys::socket, unistd::close};

use super::NetworkConfigurationError;

ioctl_write_ptr_bad!(siocsifflags, libc::SIOCSIFFLAGS, libc::ifreq);
ioctl_write_ptr_bad!(siocsifaddr, libc::SIOCSIFADDR, libc::ifreq);
ioctl_write_ptr_bad!(siocsifnetmask, libc::SIOCSIFNETMASK, libc::ifreq);
ioctl_write_ptr_bad!(siocaddrt, libc::SIOCADDRT, libc::rtentry);

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
    pub(crate) fn new(iface: String) -> Result<ConfigSocket, NetworkConfigurationError> {
        if iface.len() >= libc::IFNAMSIZ {
            return Err(NetworkConfigurationError::new(format!(
                "Interface name '{}' exceeds max length of {}",
                iface,
                libc::IFNAMSIZ
            )));
        }

        let fd = match socket::socket(
            socket::AddressFamily::Inet,
            socket::SockType::Datagram,
            socket::SockFlag::empty(),
            None,
        ) {
            Ok(fd) => fd,
            Err(err) => {
                return Err(NetworkConfigurationError::new(format!(
                    "Failed to create config socket: {}",
                    err
                )));
            }
        };

        Ok(ConfigSocket { fd, iface })
    }

    unsafe fn request(&self) -> libc::ifreq {
        let mut req: libc::ifreq = mem::zeroed();
        ptr::copy_nonoverlapping(
            self.iface.as_ptr() as *const libc::c_char,
            req.ifr_name.as_mut_ptr(),
            self.iface.len(),
        );

        req
    }

    pub(crate) fn enable(&self, value: bool) -> Result<(), NetworkConfigurationError> {
        unsafe {
            let mut req = self.request();

            if let Err(err) = siocsifflags(self.fd, &mut req) {
                return Err(NetworkConfigurationError::new(format!(
                    "Failed to get interface flags: {}",
                    err
                )));
            }

            if value {
                req.ifr_ifru.ifru_flags |= libc::IFF_UP as i16 | libc::IFF_UP as i16;
            } else {
                req.ifr_ifru.ifru_flags &= !libc::IFF_UP as i16;
            }

            if let Err(err) = siocsifflags(self.fd, &req) {
                return Err(NetworkConfigurationError::new(format!(
                    "Failed to set interface flags: {}",
                    err
                )));
            }
        }

        Ok(())
    }

    pub(crate) fn set_ip(&self, addr: IpAddr) -> Result<(), NetworkConfigurationError> {
        let ip = match addr {
            IpAddr::V4(ip) => ip,
            IpAddr::V6(_) => {
                return Err(NetworkConfigurationError::new(
                    "IPv6 is not supported".to_string(),
                ));
            }
        };

        unsafe {
            let mut req = self.request();

            req.ifr_ifru.ifru_addr.sa_family = libc::AF_INET as u16;
            ip.octets().iter().enumerate().for_each(|(i, octet)| {
                // offset by `libc::AF_*` size
                req.ifr_ifru.ifru_addr.sa_data[i + mem::size_of::<u16>()] = *octet as i8;
            });

            if let Err(err) = siocsifaddr(self.fd, &req) {
                return Err(NetworkConfigurationError::new(format!(
                    "Failed to set interface address: {}",
                    err
                )));
            }
        }

        Ok(())
    }

    pub(crate) fn set_netmask(&self, netmask: IpAddr) -> Result<(), NetworkConfigurationError> {
        let ip = match netmask {
            IpAddr::V4(ip) => ip,
            IpAddr::V6(_) => {
                return Err(NetworkConfigurationError::new(
                    "IPv6 is not supported".to_string(),
                ));
            }
        };

        unsafe {
            let mut req = self.request();

            req.ifr_ifru.ifru_netmask.sa_family = libc::AF_INET as u16;
            ip.octets().iter().enumerate().for_each(|(i, octet)| {
                // offset by `libc::AF_*` size
                req.ifr_ifru.ifru_netmask.sa_data[i + mem::size_of::<u16>()] = *octet as i8;
            });

            if let Err(err) = siocsifnetmask(self.fd, &req) {
                return Err(NetworkConfigurationError::new(format!(
                    "Failed to set interface netmask: {}",
                    err
                )));
            }
        }

        Ok(())
    }

    pub(crate) fn set_gateway(&self, gateway: IpAddr) -> Result<(), NetworkConfigurationError> {
        let ip = match gateway {
            IpAddr::V4(ip) => ip,
            IpAddr::V6(_) => {
                return Err(NetworkConfigurationError::new(
                    "IPv6 is not supported".to_string(),
                ));
            }
        };

        let mut rt: libc::rtentry = unsafe { mem::zeroed() };

        rt.rt_flags = libc::RTF_UP | libc::RTF_GATEWAY;
        rt.rt_gateway.sa_family = libc::AF_INET as u16;
        ip.octets().iter().enumerate().for_each(|(i, octet)| {
            // offset by `libc::AF_*` size
            rt.rt_gateway.sa_data[i + mem::size_of::<u16>()] = *octet as i8;
        });

        rt.rt_dst = libc::sockaddr {
            sa_family: libc::AF_INET as u16,
            sa_data: [0; 14],
        };
        rt.rt_genmask = libc::sockaddr {
            sa_family: libc::AF_INET as u16,
            sa_data: [0; 14],
        };

        let c_str = CString::new(self.iface.clone()).unwrap();
        let c_world: *mut i8 = c_str.as_ptr() as *mut i8;

        rt.rt_dev = c_world;
        unsafe {
            if let Err(err) = siocaddrt(self.fd, &rt) {
                return Err(NetworkConfigurationError::new(format!(
                    "Failed to set interface gateway: {}",
                    err
                )));
            }
        }

        Ok(())
    }
}

/// Configures the DNS server
///
/// This is done by writeing to the `/etc/resolv.conf` file.
///
/// # Arguments
///
/// * `addr`: The dns ip addres to use
pub(crate) fn set_dns(addr: IpAddr) -> Result<(), NetworkConfigurationError> {
    if let Err(err) = fs::write("/etc/resolv.conf", format!("nameserver {}", addr)) {
        return Err(NetworkConfigurationError::new(format!(
            "Failed configuring DNS: {}",
            err
        )));
    }

    Ok(())
}
