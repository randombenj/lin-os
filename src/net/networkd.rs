/// Network configuration daemon.

use std::net::{IpAddr, Ipv4Addr};

use log::debug;

use super::iface::{get_network_interfaces, NetworkInterface};
use super::NetworkError;

pub fn configure_network() -> Result<(), NetworkError> {
    // TODO: wait for carrier?
    let network_config = vec![
        NetworkInterface {
            name: "lo".to_string(),
            mac: None, // set by kernel
            ip: Some(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))),
            netmask: Some(IpAddr::V4(Ipv4Addr::new(255, 0, 0, 0))),
            gateway: Some(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))),
        },
        // NetworkInterface {
        //     name: "eth0".to_string(),
        //     dhcp: false,
        //     mac: None, // set by kernel
        //     ip: Some(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100))),
        //     netmask: Some(IpAddr::V4(Ipv4Addr::new(55, 255, 255, 0))),
        //     gateway: Some(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1))),
        // },
        NetworkInterface {
            name: "eth0".to_string(),
            mac: None, // set by kernel
            ip: None,
            netmask: None,
            gateway: None,
        },
    ];

    for interface in &network_config {
        debug!("configuring {:?}", interface);
        if let Err(err) = interface.configure() {
            return Err(err);
        }
    }

    if let Ok(interfaces) = get_network_interfaces() {
        debug!("configured interfaces: {:?}", interfaces);
    }

    Ok(())
}
