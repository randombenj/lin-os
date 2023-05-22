use log::{debug, trace};
use rand::{self, Rng};
use std::{
    io::{self, Error},
    net::{IpAddr, Ipv4Addr},
    time::{Duration, Instant},
};

use dhcproto::{v4, Decodable, Decoder, Encodable, Encoder};
use pnet::{
    datalink::{self, Channel, Config, NetworkInterface},
    packet::{
        ethernet::{EtherTypes, EthernetPacket, MutableEthernetPacket},
        ip::IpNextHeaderProtocols,
        ipv4::{self, Ipv4Packet, MutableIpv4Packet},
        udp::{self, MutableUdpPacket, UdpPacket},
        Packet,
    },
    util::MacAddr,
};

use super::iface::StaticNetworkInterfaceConfig;

pub const IPV4_HEADER_LENGTH: u8 = 20;

/// Creates a default dhcpv4 message.
///
/// The message asks for the following options:
/// - SubnetMask
/// - Router
/// - DomainNameServer
/// - DomainName
///
/// # Arguments
///
/// * `mac` - The mac address of the interface.
/// * `dhcp_message_type` - The type of the dhcp message.
fn create_dhcpv4_message(mac: MacAddr, dhcp_message_type: v4::MessageType) -> v4::Message {
    // construct a new Message
    let chaddr = mac.octets();

    let mut msg = v4::Message::default();
    msg.set_flags(v4::Flags::default().set_broadcast()) // set broadcast to true
        .set_chaddr(&chaddr) // set chaddr
        .opts_mut()
        .insert(v4::DhcpOption::MessageType(dhcp_message_type)); // set msg type

    // set some more options
    msg.opts_mut()
        .insert(v4::DhcpOption::ParameterRequestList(vec![
            v4::OptionCode::SubnetMask,
            v4::OptionCode::Router,
            v4::OptionCode::DomainNameServer,
            v4::OptionCode::DomainName,
        ]));
    msg.opts_mut()
        .insert(v4::DhcpOption::ClientIdentifier(chaddr.to_vec()));
    msg
}

/// Creates a dhcp udp packet from a dhcp message.
///
/// The packet is wrapped in an udp packet, ipv4 packet and then in an ethernet packet.
///
/// # Arguments
///
/// * `dhcp_message` - The dhcp message to put into an ethernet frame.
fn create_dhcp_packet(dhcp_message: v4::Message) -> io::Result<EthernetPacket<'static>> {
    // the mac address is required to do a dhcp request
    let mac = dhcp_message.chaddr();

    let payload = dhcp_message.to_vec().unwrap();

    // -- UDP packet
    let buf = vec![0; 8 + payload.len()];
    let mut udp_packet = MutableUdpPacket::owned(buf).unwrap();

    udp_packet.set_source(68);
    udp_packet.set_destination(67);
    udp_packet.set_length((8 + payload.len()) as u16);
    udp_packet.set_payload(&payload);

    let dst_ip = Ipv4Addr::new(255, 255, 255, 255);
    let src_ip = Ipv4Addr::new(0, 0, 0, 0);

    udp_packet.set_checksum(udp::ipv4_checksum(
        &udp_packet.to_immutable(),
        &src_ip,
        &dst_ip,
    ));

    let payload = udp_packet.packet();

    // -- IPv4 packet
    let buf = vec![0; IPV4_HEADER_LENGTH as usize + payload.len()];

    let mut ip_packet = MutableIpv4Packet::owned(buf).unwrap();
    ip_packet.set_version(4);
    ip_packet.set_header_length(IPV4_HEADER_LENGTH / 4);
    ip_packet.set_dscp(0);
    ip_packet.set_ecn(0);
    ip_packet.set_total_length(IPV4_HEADER_LENGTH as u16 + payload.len() as u16);
    ip_packet.set_identification(rand::thread_rng().gen());
    ip_packet.set_flags(0);
    ip_packet.set_fragment_offset(0);
    ip_packet.set_ttl(64);
    ip_packet.set_next_level_protocol(IpNextHeaderProtocols::Udp);
    ip_packet.set_source(src_ip);
    ip_packet.set_destination(dst_ip);
    ip_packet.set_options(&Vec::new());
    ip_packet.set_payload(payload);

    ip_packet.set_checksum(ipv4::checksum(&ip_packet.to_immutable()));

    // -- Ethernet frame
    let payload = ip_packet.packet();
    let buf = vec![0u8; EthernetPacket::minimum_packet_size() + payload.len()];
    let mut ethernet_packet = MutableEthernetPacket::owned(buf).unwrap();

    let dst_mac = MacAddr::broadcast();
    let src_mac = match *mac {
        [a, b, c, d, e, f] => MacAddr::new(a, b, c, d, e, f),
        _ => {
            return Err(Error::new(
                io::ErrorKind::Other,
                format!("Invalid MAC address: {:?}", mac),
            ))
        }
    };

    ethernet_packet.set_destination(dst_mac);
    ethernet_packet.set_source(src_mac);
    ethernet_packet.set_ethertype(EtherTypes::Ipv4);
    ethernet_packet.set_payload(payload);

    Ok(ethernet_packet.consume_to_immutable())
}

/// Receives a dhcp message from the given interface.
///
/// The message is received and then unwrapped from an ethernet frame,
/// ipv4 frame and udp frame.
///
/// # Arguments
///
/// * `interface` - The interface to receive the message on.
fn receive_message(interface: NetworkInterface) -> io::Result<v4::Message> {
    let (_, mut receiver) = match datalink::channel(&interface, Config::default()) {
        Ok(Channel::Ethernet(tx, rx)) => (tx, rx),
        Ok(_) => return Err(Error::new(io::ErrorKind::Other, "Unknown channel type")),
        Err(err) => return Err(err),
    };

    let timeout = Duration::from_secs(10);
    let start_time = Instant::now();

    let msg = loop {
        if Instant::now().duration_since(start_time) > timeout {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "Timeout waiting for OFFER",
            ));
        }

        let buf = receiver
            .next()
            .map_err(|e| format!("Error receiving packets: {}", e))
            .unwrap();

        // -- Ethernet frame
        let ether_packet = match EthernetPacket::new(&buf[..]) {
            Some(ether_packet) => ether_packet,
            None => continue,
        };

        if ether_packet.get_ethertype() != EtherTypes::Ipv4 {
            continue;
        }

        // -- IPv4 packet
        let ip_packet = match Ipv4Packet::new(ether_packet.payload()) {
            Some(ip_packet) => ip_packet,
            None => continue,
        };

        if ip_packet.get_next_level_protocol() != IpNextHeaderProtocols::Udp {
            continue;
        }

        // -- UDP packet
        let udp_packet = match UdpPacket::new(ip_packet.payload()) {
            Some(udp_packet) => udp_packet,
            None => continue,
        };

        if udp_packet.get_destination() != 68 {
            debug!("Received packet on port {}", udp_packet.get_destination());
            continue;
        }

        let input = udp_packet.payload();

        let msg = v4::Message::decode(&mut Decoder::new(&input)).unwrap();

        // now encode
        let mut buf = Vec::new();
        let mut e = Encoder::new(&mut buf);
        msg.encode(&mut e).unwrap();

        break msg;
    };

    Ok(msg)
}

/// Sends a DHCP discover message from the given interface.
///
/// See: https://www.ietf.org/rfc/rfc2131.txt
///
/// # Arguments
///
/// * `interface` - The interface to send the message from.
///
/// # Returns
///
/// * `io::Result<v4::Message>` - The DHCP offer message.
fn dhcp_discover(interface: NetworkInterface) -> io::Result<v4::Message> {
    let mac = match interface.mac {
        Some(mac) => mac,
        None => return Err(Error::new(io::ErrorKind::NotFound, "No MAC address found")),
    };

    let (mut sender, _) = match datalink::channel(&interface, Config::default()) {
        Ok(Channel::Ethernet(tx, rx)) => (tx, rx),
        Ok(_) => panic!("Error creating channel: Unknown channel type"),
        Err(err) => return Err(err),
    };

    // -- DHCP discover message
    let msg = create_dhcpv4_message(mac, v4::MessageType::Discover);
    let dhcp_discover_packet = create_dhcp_packet(msg)?;
    let dhcp_discover_packet = dhcp_discover_packet.packet();

    sender.send_to(dhcp_discover_packet, Some(interface.clone()));
    debug!("DISCOVER from {}", mac);

    let msg = receive_message(interface)?;
    trace!("DISCOVER response: {}", msg);

    Ok(msg)
}

/// Sends a DHCP request message from the given interface.
///
/// See: https://www.ietf.org/rfc/rfc2131.txt
///
/// # Arguments
///
/// * `interface` - The interface to send the message from.
/// * `discover_response` - The DHCP discover response message.
///   Obtained from `dhcp_discover`.
///
/// # Returns
///
/// * `io::Result<v4::Message>` - The DHCP ack message.
fn dhcp_request(
    interface: NetworkInterface,
    discover_response: v4::Message,
) -> io::Result<v4::Message> {
    let mac = match interface.mac {
        Some(mac) => mac,
        None => return Err(Error::new(io::ErrorKind::NotFound, "No MAC address found")),
    };

    let (mut sender, _) = match datalink::channel(&interface, Config::default()) {
        Ok(Channel::Ethernet(tx, rx)) => (tx, rx),
        Ok(_) => panic!("Error creating channel: Unknown channel type"),
        Err(err) => return Err(err),
    };

    // -- DHCP request message
    let mut msg = create_dhcpv4_message(mac, v4::MessageType::Request);
    msg.opts_mut().insert(v4::DhcpOption::RequestedIpAddress(
        discover_response.yiaddr(),
    ));
    msg.opts_mut()
        .insert(v4::DhcpOption::ServerIdentifier(discover_response.siaddr()));

    let dhcp_discover_packet = create_dhcp_packet(msg)?;
    let dhcp_discover_packet = dhcp_discover_packet.packet();

    sender.send_to(dhcp_discover_packet, Some(interface.clone()));
    debug!("REQUEST ip {} from {}", discover_response.yiaddr(), mac);

    let msg = receive_message(interface)?;
    trace!("REQUEST response: {}", msg);

    Ok(msg)
}

/// Request an IP address from a DHCP server.
///
/// # Arguments
///
/// * `iface_name` - The name of the interface to request an IP address for.
///
/// # Example
///
/// ```rust
/// use dhcp::request;
///
/// let iface_name = "eth0".to_string();
/// let iface = request(&iface_name).unwrap();
/// ```
pub fn request(iface_name: &String) -> io::Result<StaticNetworkInterfaceConfig> {
    // TODO: add some retry logic in case of faillures and timeouts

    // check if the interface exists and is up
    let interface = match datalink::interfaces()
        .into_iter()
        .filter(|i| &i.name == iface_name)
        .next()
    {
        Some(interface) => interface,
        None => {
            return Err(Error::new(
                io::ErrorKind::NotFound,
                format!("Interface with name {} not found", iface_name),
            ))
        }
    };
    if !interface.is_up() {
        return Err(Error::new(
            io::ErrorKind::NotFound,
            format!("Interface {} is not up", iface_name),
        ));
    }

    // -- do the dhcp request
    let discover_response = dhcp_discover(interface.clone())?;
    let request_response = dhcp_request(interface.clone(), discover_response)?;

    // assemble a static network interface config
    // from the dhcp response
    let netmask = match request_response.opts().get(v4::OptionCode::SubnetMask) {
        Some(v4::DhcpOption::SubnetMask(netmask)) => IpAddr::V4(*netmask),
        _ => {
            return Err(Error::new(
                io::ErrorKind::NotFound,
                format!("{}: no netmask returned by dhcp.", iface_name),
            ))
        }
    };

    let gateway = match request_response.opts().get(v4::OptionCode::Router) {
        Some(v4::DhcpOption::Router(router)) => match router.first() {
            Some(r) => IpAddr::V4(*r),
            None => {
                return Err(Error::new(
                    io::ErrorKind::NotFound,
                    format!("{}: no gateway returned by dhcp.", iface_name),
                ))
            }
        },
        _ => {
            return Err(Error::new(
                io::ErrorKind::NotFound,
                format!("{}: no gateway returned by dhcp.", iface_name),
            ))
        }
    };

    return Ok(StaticNetworkInterfaceConfig {
        name: interface.name,
        ip: IpAddr::V4(request_response.yiaddr()),
        netmask: netmask,
        gateway: gateway,
    });
}
