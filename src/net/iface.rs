use std::net::IpAddr;

use super::{
    iface_config::{set_dns, ConfigSocket},
    NetworkConfigurationError,
};
use crate::net::dhcp;

#[derive(Debug)]
pub struct StaticNetworkInterfaceConfig {
    pub name: String,
    pub ip: IpAddr,
    pub netmask: IpAddr,
    pub gateway: IpAddr,
    pub dns: Option<IpAddr>,
}

#[derive(Debug)]
pub struct DynamicNetworkInterfaceConfig {
    pub name: String,
}

/// A network iface config, either static or dhcp.
///
/// This enum can contain either an [`StatcInterfaceConfig`] or a [`DynamicInterfaceConfig`], see their
/// respective documentation for more details.
///
/// # Examples
///
/// ```
/// let network_config = vec![
///     NetworkInterfaceConfig::Static(StaticNetworkInterfaceConfig {
///         name: "lo".to_string(),
///         ip: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
///         netmask: IpAddr::V4(Ipv4Addr::new(255, 0, 0, 0)),
///         gateway: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
///     }),
///     NetworkInterfaceConfig::Dynamic(DynamicNetworkInterfaceConfig {
///         name: "eth0".to_string(),
///     }),
/// ];
///
/// network_config.iter().for_each(|config| {
///   config.apply();
/// });
/// ```
#[derive(Debug)]
pub enum NetworkInterfaceConfig {
    Static(StaticNetworkInterfaceConfig),
    Dynamic(DynamicNetworkInterfaceConfig),
}

pub trait NetworkInterfaceConfigApply {
    fn apply(&self) -> Result<(), NetworkConfigurationError>;
}

impl NetworkInterfaceConfigApply for NetworkInterfaceConfig {
    fn apply(&self) -> Result<(), NetworkConfigurationError> {
        match self {
            NetworkInterfaceConfig::Static(config) => config.apply(),
            NetworkInterfaceConfig::Dynamic(config) => config.apply(),
        }
    }
}

impl NetworkInterfaceConfigApply for StaticNetworkInterfaceConfig {
    fn apply(&self) -> Result<(), NetworkConfigurationError> {
        let iface = pnet::datalink::interfaces()
            .into_iter()
            .find(|iface| iface.name == self.name)
            .ok_or_else(|| {
                NetworkConfigurationError::new(format!("Interface '{}' not found", self.name))
            })?;

        let config = ConfigSocket::new(self.name.clone())?;
        config.enable(true)?;
        config.set_ip(self.ip)?;
        config.set_netmask(self.netmask)?;
        if !iface.is_loopback() {
            config.set_gateway(self.gateway)?;
        }
        if self.dns.is_some() {
            set_dns(self.dns.unwrap())?;
        }

        Ok(())
    }
}

impl NetworkInterfaceConfigApply for DynamicNetworkInterfaceConfig {
    fn apply(&self) -> Result<(), NetworkConfigurationError> {
        let config = ConfigSocket::new(self.name.clone())?;
        config.enable(true)?;

        let static_interface_config = match dhcp::request(&self.name) {
            Ok(config) => config,
            Err(err) => {
                return Err(NetworkConfigurationError::new(format!(
                    "DHCP config failed: {}",
                    err
                )))
            }
        };
        static_interface_config.apply()?;

        Ok(())
    }
}
