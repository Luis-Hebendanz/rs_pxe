#![allow(clippy::option_map_unit_fn)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(unused_mut)]
#![allow(dead_code)]
mod cli_opts;

use log::*;
use smoltcp::iface::Config;
use smoltcp::iface::Routes;
use smoltcp::phy::wait as phy_wait;
use smoltcp::phy::Checksum;
use smoltcp::phy::Device;
use smoltcp::phy::Medium;
use smoltcp::phy::RawSocket;
use smoltcp::phy::RxToken;
use smoltcp::phy::TxToken;
use smoltcp::time::Instant;
use smoltcp::wire::DhcpMessageType;
use smoltcp::wire::DhcpPacket;
use smoltcp::wire::EthernetAddress;
use smoltcp::wire::EthernetFrame;
use smoltcp::wire::HardwareAddress;

use rand::prelude::*;
use smoltcp::wire::IpAddress;
use smoltcp::wire::IpCidr;
use smoltcp::wire::Ipv4Address;
use smoltcp::wire::Ipv4Packet;
use smoltcp::wire::UdpPacket;
use smoltcp::{iface::Interface, phy::ChecksumCapabilities};
use std::borrow::BorrowMut;
use std::collections::BTreeMap;
use std::convert::TryFrom;
use std::io::Read;
use std::os::unix::io::AsRawFd;
use std::str::FromStr;
use uuid::Uuid;

use crate::dhcp_options::DhcpOption;
use prelude::*;
use rs_pxe::*;

//RFC: https://datatracker.ietf.org/doc/html/rfc2132
fn main() {
    cli_opts::setup_logging("");
    info!("Starting pxe....");

    let (mut opts, mut _free) = cli_opts::create_options();

    let mut matches = cli_opts::parse_options(&opts, _free);
    let t = &matches.opt_str("mac").unwrap();
    let hardware_addr: &EthernetAddress = &EthernetAddress::from_str(t).unwrap();
    let t = &matches.opt_str("ip").unwrap();
    let ip = &matches
        .opt_get_default("ip", IpAddress::from_str(t).unwrap())
        .unwrap();
    let ip_addrs = [IpCidr::new(*ip, 24)];

    if matches.opt_present("raw") {
        let interface = matches.opt_str("raw").unwrap();
        let mut device = RawSocket::new(&interface, Medium::Ethernet).unwrap();

        // Create interface
        let mut config = match device.capabilities().medium {
            Medium::Ethernet => Config::new(Into::into(*hardware_addr)),
            Medium::Ip => Config::new(smoltcp::wire::HardwareAddress::Ip),
            Medium::Ieee802154 => todo!(),
        };
        config.random_seed = rand::random();

        let mut iface = Interface::new(config, &mut device);

        iface.update_ip_addrs(|ip_addr| {
            ip_addr.push(IpCidr::new(*ip, 24)).unwrap();
        });

        server(&mut device, &mut iface);
    } else if matches.opt_present("tun") || matches.opt_present("tap") {
        let mut device = cli_opts::parse_tuntap_options(&mut matches);

        // Create interface
        let mut config = match device.capabilities().medium {
            Medium::Ethernet => Config::new(Into::into(*hardware_addr)),
            Medium::Ip => Config::new(smoltcp::wire::HardwareAddress::Ip),
            Medium::Ieee802154 => todo!(),
        };
        config.random_seed = rand::random();
        let mut iface = Interface::new(config, &mut device);

        iface.update_ip_addrs(|ip_addr| {
            ip_addr.push(IpCidr::new(*ip, 24)).unwrap();
        });

        server(&mut device, &mut iface);
    } else {
        let brief = "Either --raw or --tun or --tap must be specified";
        panic!("{}", opts.usage(brief));
    };
}

pub fn server<DeviceT: AsRawFd>(device: &mut DeviceT, iface: &mut Interface)
where
    DeviceT: for<'d> Device,
{
    log::info!("Starting server");
    let fd = device.as_raw_fd();
    let server_mac_address = match iface.hardware_addr() {
        HardwareAddress::Ethernet(addr) => addr,
        _ => panic!("Currently we only support ethernet"),
    };
    let server_ip = iface.ipv4_addr().unwrap();
    let mut checksum = ChecksumCapabilities::ignored();
    checksum.ipv4 = Checksum::Both;
    checksum.udp = Checksum::Both;

    loop {
        let time = Instant::now();
        phy_wait(fd, None).unwrap();
        let (rx_token, tx_token) = device.receive(time).unwrap();
        let mut client_uuid: Option<Uuid> = None;
        let mut system_arches: Vec<ClientArchType> = vec![];
        let mut vendor_id: Option<String> = None;
        let mut client_mac_address: Option<EthernetAddress> = None;
        let mut transaction_id: Option<u16> = None;
        let mut secs = 0;
        rx_token
            .consume(rs_pxe::parse::pxe_recv)
            .map_err(|e| match e {
                Error::Ignore(e) => {
                    debug!("Ignored packet. Reason: {}", e);
                    Ok(())
                }
                e => Err(e),
            })
            .unwrap();
        // rx_token
        //     .consume(Instant::now(), |buffer| {
        //         let ether = EthernetFrame::new_checked(&buffer).unwrap();

        //         if ether.dst_addr() == EthernetAddress::BROADCAST {
        //             if ether.src_addr() != EthernetAddress([0x00, 0x01, 0x2e, 0x91, 0xf7, 0xfd]) {
        //                 return Ok(());
        //             }

        //             println!("{}", ether);
        //             let ipv4 = match Ipv4Packet::new_checked(ether.payload()) {
        //                 Ok(i) => i,
        //                 Err(e) => {
        //                     error!("Parsing ipv4 packet failed: {}", e);
        //                     return Ok(());
        //                 }
        //             };

        //             if ipv4.dst_addr() != Ipv4Address::BROADCAST {
        //                 error!("Not broadcast in ipv4 address");
        //                 return Ok(());
        //             }

        //             let udp = match UdpPacket::new_checked(ipv4.payload()) {
        //                 Ok(i) => i,
        //                 Err(e) => {
        //                     error!("Parsing udp packet failed: {}", e);
        //                     return Ok(());
        //                 }
        //             };

        //             if udp.dst_port() != 67 {
        //                 error!("Udp packet does not have dst port 67");
        //                 return Ok(());
        //             }

        //             let dhcp = match DhcpPacket::new_checked(udp.payload()) {
        //                 Ok(i) => i,
        //                 Err(e) => {
        //                     error!("Parsing dhcp packet failed: {}", e);
        //                     return Ok(());
        //                 }
        //             };

        //             if !dhcp.flags().contains(DhcpFlags::BROADCAST) {
        //                 error!("Not a BOOTP dhcp packet");
        //                 return Ok(());
        //             }
        //             secs = dhcp.secs();
        //             let mut next = dhcp.options().unwrap();
        //             let mut option;

        //             loop {
        //                 (next, option) = DhcpOption::parse(next).unwrap();

        //                 if let DhcpOption::ClientArchTypeList(data) = option {
        //                     let (prefix, body, suffix) = unsafe { data.align_to::<u16>() };
        //                     if !prefix.is_empty() || !suffix.is_empty() {
        //                         error!("Invalid arch type list. Improperly aligned");
        //                         return Err(Error::Malformed);
        //                     }
        //                     system_arches = body
        //                         .iter()
        //                         .map(|&i| PxeArchType::try_from(u16::from_be(i)).unwrap())
        //                         .collect();
        //                 }

        //                 if let DhcpOption::ClientMachineId(id) = option {
        //                     client_uuid = Some(Uuid::from_slice(id.id).unwrap());
        //                 }

        //                 if let DhcpOption::VendorClassId(vendor) = option {
        //                     vendor_id = Some(vendor.to_string());
        //                 }

        //                 if option == DhcpOption::EndOfList {
        //                     break;
        //                 }
        //             }

        //             client_mac_address = Some(dhcp.client_hardware_address());
        //             transaction_id = Some(dhcp.transaction_id());
        //         }
        //         Ok(())
        //     })
        //     .unwrap();
        // info!("Hello 1");
        // let tx_token = device.transmit().unwrap();
        // let mut client_uuid = Some(Uuid::default());
        // let mut system_arches: Vec<PxeArchType> = vec![PxeArchType::X86PC; 1];
        // let mut vendor_id: Option<String> = Some("asdasdasd".to_string());
        // let mut client_mac_address = Some(EthernetAddress::from_str(DEFAULT_MAC).unwrap());
        // let mut transaction_id = Some(0x12345);
        if let Some(client_mac_address) = client_mac_address {
            info!("Client mac address: {}", client_mac_address);
            info!("Supported system arches: {:#?}", system_arches);
            info!("Client guid: {}", client_uuid.unwrap().hyphenated());
            info!("Client vendor id: {}", vendor_id.unwrap());

            //     tx_token
            //         .consume(Instant::now(), 300, |buffer| {
            //             const IP_NULL: Ipv4Address = Ipv4Address([0, 0, 0, 0]);
            //             let dhcp_packet = DhcpRepr {
            //                 message_type: DhcpMessageType::Offer,
            //                 transaction_id: transaction_id.unwrap(),
            //                 client_hardware_address: client_mac_address,
            //                 secs: secs,
            //                 client_ip: IP_NULL,
            //                 your_ip: IP_NULL,
            //                 server_ip: IP_NULL,
            //                 broadcast: true,
            //                 sname: None,
            //                 boot_file: None,
            //                 relay_agent_ip: IP_NULL,

            //                 // unimportant
            //                 router: None,
            //                 subnet_mask: None,
            //                 requested_ip: None,
            //                 client_identifier: None,
            //                 server_identifier: None,
            //                 parameter_request_list: None,
            //                 dns_servers: None,
            //                 max_size: None,
            //                 lease_duration: None,
            //                 client_arch_list: None,
            //                 client_interface_id: None,
            //                 client_machine_id: None,
            //                 time_offset: None,
            //                 vendor_class_id: None,
            //             };

            //             let udp_packet = UdpRepr {
            //                 src_port: 67,
            //                 dst_port: 68,
            //             };

            //             let mut packet = EthernetFrame::new_unchecked(buffer);
            //             let eth_packet = EthernetRepr {
            //                 dst_addr: EthernetAddress::BROADCAST,
            //                 src_addr: server_mac_address,
            //                 ethertype: EthernetProtocol::Ipv4,
            //             };
            //             eth_packet.emit(&mut packet);

            //             let mut packet = Ipv4Packet::new_unchecked(packet.payload_mut());
            //             let ip_packet = Ipv4Repr {
            //                 src_addr: server_ip,
            //                 dst_addr: Ipv4Address::BROADCAST,
            //                 protocol: IpProtocol::Udp,
            //                 hop_limit: 128,
            //                 payload_len: dhcp_packet.buffer_len() + udp_packet.header_len(),
            //             };
            //             ip_packet.emit(&mut packet, &checksum);

            //             let mut packet = UdpPacket::new_unchecked(packet.payload_mut());
            //             udp_packet.emit(
            //                 &mut packet,
            //                 &server_ip.into(),
            //                 &Ipv4Address::BROADCAST.into(),
            //                 dhcp_packet.buffer_len(),
            //                 |buf| {
            //                     let mut packet = DhcpPacket::new_unchecked(buf);
            //                     dhcp_packet.emit(&mut packet).unwrap();
            //                 },
            //                 &checksum,
            //             );

            //             info!("Sending DHCP offer...");
            //             Ok(())
            //         })
            //         .unwrap();
        }
    }
}
