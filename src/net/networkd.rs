/// Network configuration daemon.
use std::{
    fs,
    net::{IpAddr, Ipv4Addr},
};

use log::{debug, error, trace};
use pnet::datalink;

use crate::net::iface::NetworkInterfaceConfigApply;

use super::{
    iface::{DynamicNetworkInterfaceConfig, NetworkInterfaceConfig, StaticNetworkInterfaceConfig},
    NetworkConfigurationError,
};

pub fn configure_network() -> Result<(), NetworkConfigurationError> {
    // TODO: read from config file
    let network_config = vec![
        NetworkInterfaceConfig::Static(StaticNetworkInterfaceConfig {
            name: "lo".to_string(),
            ip: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            netmask: IpAddr::V4(Ipv4Addr::new(255, 0, 0, 0)),
            gateway: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            dns: None,
        }),
        NetworkInterfaceConfig::Dynamic(DynamicNetworkInterfaceConfig {
            name: "eth0".to_string(),
        }),
    ];

    let hosts = "127.0.0.1 localhost\n::1 localhost\n";
    if let Err(err) = fs::write("/etc/hosts", hosts) {
        return Err(NetworkConfigurationError::new(format!(
            "Failed configuring '/etc/hosts': {}",
            err
        )));
    }

    for config in network_config {
        trace!("Applying config {:?}", config);
        if let Err(err) = config.apply() {
            let name = match config {
                NetworkInterfaceConfig::Dynamic(cfg) => cfg.name,
                NetworkInterfaceConfig::Static(cfg) => cfg.name,
            };
            error!("Failed configuring '{}': {}", name, err);
            // TODO: retry config ...
        }
    }

    datalink::interfaces().iter().for_each(|iface| {
        debug!("Configured interfaces: {:?}", iface);
    });

    Ok(())
}
