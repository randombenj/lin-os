use std::net::{IpAddr, Ipv4Addr};

use crate::net::dhcp;

use super::{iface_config::ConfigSocket, NetworkError};

#[derive(Debug)]
pub struct NetworkInterface {
    pub name: String,
    pub mac: Option<[u8; 6]>,
    pub ip: Option<IpAddr>,
    pub netmask: Option<IpAddr>,
    pub gateway: Option<IpAddr>,
}

/// Get a list of network interfaces
/// that are actually configured on the system.
pub fn get_network_interfaces() -> Result<Vec<NetworkInterface>, NetworkError> {
    let mut interfaces: Vec<NetworkInterface> = Vec::new();

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
        // -- find or create interface
        let iface: &mut NetworkInterface = match interfaces
            .iter_mut()
            .find(|i| i.name == ifaddr.interface_name)
        {
            Some(iface) => iface,
            None => {
                let interface = NetworkInterface {
                    name: ifaddr.interface_name,
                    mac: None,
                    ip: None,
                    netmask: None,
                    gateway: None,
                };
                interfaces.push(interface);
                interfaces.last_mut().unwrap()
            }
        };

        // -- set interface properties
        let address = match ifaddr.address {
            Some(address) => address,
            None => continue,
        };

        if let Some(ipv4) = address.as_sockaddr_in() {
            let [a, b, c, d] = ipv4.ip().to_be_bytes();
            iface.ip = Some(IpAddr::V4(Ipv4Addr::new(a, b, c, d)));
        }

        if let Some(_ipv6) = address.as_sockaddr_in6() {
            // TODO: iface.ip = Some(IpAddr::V6(ipv6.ip().clone()));
            continue;
        }

        if let Some(mac) = address.as_link_addr() {
            iface.mac = mac.addr();
        }
    }

    Ok(interfaces)
}

impl NetworkInterface {
    /// Configures to the given interface.
    /// This function will enable the interface and set the IP address.
    pub fn configure(&self) -> Result<(), NetworkError> {
        let config = ConfigSocket::new(self.name.clone())?;
        config.enable(true)?;

        // configure ip address statically
        // or via dhcp if no ip address is given
        match self.ip {
            Some(ip) => config.set_ip(ip)?,
            None => dhcp::request(&self.name).unwrap(),
        }

        Ok(())
    }
}
