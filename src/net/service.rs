use core::fmt;
use std::net::{IpAddr, Ipv4Addr};

use super::iface::ConfigSocket;

#[derive(Debug, Clone)]
pub struct NetworkError {
    pub message: String,
    pub err: nix::Error,
}

impl fmt::Display for NetworkError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "mount error: {} -> {}", self.message, self.err)
    }
}

fn configure_loopback() -> Result<(), NetworkError> {
    let config = match ConfigSocket::new("lo".to_string()) {
        Ok(config) => config,
        Err(err) => return Err(err),
    };

    if let Err(err) = config.enable(true) {
        return Err(err);
    }

    let addr = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
    if let Err(err) = config.set_ip(addr) {
        return Err(err);
    }

    Ok(())
}

fn list_interfaces() -> Result<(), NetworkError>  {
    let addrs = match nix::ifaddrs::getifaddrs() {
        Ok(addrs) => addrs,
        Err(err) => {
            return Err(NetworkError {
                message: "failed to get interface addresses".to_string(),
                err: err,
            });
        }
    };

    for ifaddr in addrs {
        match ifaddr.address {
            Some(address) => {
                println!("interface {} address {}", ifaddr.interface_name, address);
            }
            None => {
                println!(
                    "interface {} with unsupported address family",
                    ifaddr.interface_name
                );
            }
        }
    }

    Ok(())
}

pub fn configure_network() -> Result<(), NetworkError> {
    if let Err(err) = list_interfaces() {
        return Err(err);
    }

    if let Err(err) = configure_loopback() {
        return Err(err);
    }

    if let Err(err) = list_interfaces() {
        return Err(err);
    }

    Ok(())
}
