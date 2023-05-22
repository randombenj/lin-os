use log::{debug, error, trace};
use rand::{self, Rng};
use std::{
    io::{self, Error},
    net::Ipv4Addr,
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

pub const IPV4_HEADER_LENGTH: u8 = 20;

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


fn dhcp_request(interface: NetworkInterface, discover_response: v4::Message) -> io::Result<v4::Message> {
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
    msg.opts_mut()
        .insert(v4::DhcpOption::RequestedIpAddress(discover_response.yiaddr()));
    msg.opts_mut()
        .insert(v4::DhcpOption::ServerIdentifier(discover_response.siaddr()));


    let dhcp_discover_packet = create_dhcp_packet(msg)?;
    let dhcp_discover_packet = dhcp_discover_packet.packet();

    sender.send_to(dhcp_discover_packet, Some(interface.clone()));
    debug!("REQUEST from {}/{}", mac, discover_response.yiaddr());

    let msg = receive_message(interface)?;
    trace!("REQUEST response: {}", msg);

    Ok(msg)
}

pub fn request(iface_name: &String) -> io::Result<()> {
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

    let discover_response = dhcp_discover(interface.clone())?;
    let _msg = dhcp_request(interface.clone(), discover_response)?;

    return Ok(());
}
