/// Network configuration daemon.
use std::net::{IpAddr, Ipv4Addr};

use log::{debug, trace};
use nix::errno::Errno;
use pnet::datalink;

use crate::net::iface::NetworkInterfaceConfigApply;

use super::iface::{
    DynamicNetworkInterfaceConfig, NetworkInterfaceConfig, StaticNetworkInterfaceConfig,
};
use super::NetworkError;

pub fn configure_network() -> Result<(), NetworkError> {
    // TODO: read from config file
    let network_config = vec![
        NetworkInterfaceConfig::Static(StaticNetworkInterfaceConfig {
            name: "lo".to_string(),
            ip: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            netmask: IpAddr::V4(Ipv4Addr::new(255, 0, 0, 0)),
            gateway: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
        }),
        NetworkInterfaceConfig::Dynamic(DynamicNetworkInterfaceConfig {
            name: "eth0".to_string(),
        }),
    ];

    let (errors, _): (Vec<_>, Vec<_>) = network_config
        .iter()
        .map(|config| {
            trace!("Applying config {:?}", config);
            config.apply()
        })
        .partition(Result::is_err);

    let errors: Vec<_> = errors.into_iter().map(Result::unwrap_err).collect();
    if !errors.is_empty() {
        return Err(NetworkError {
            message: format!("Failed to configure network: {:?}", errors),
            err: Errno::ENETDOWN,
        });
    }

    datalink::interfaces().iter().for_each(|iface| {
        debug!("Configured interfaces: {:?}", iface);
    });

    Ok(())
}
