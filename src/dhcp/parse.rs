use log::*;

use smoltcp::wire::DhcpMessageType;
use smoltcp::wire::DhcpPacket;

use super::error::*;
use crate::dhcp::options::*;

use std::convert::TryFrom;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum FirmwareType {
    Unknown,
    IPxe,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct PxeClientInfo {
    pub client_arch: ClientArchType,
    /// Optionally identify the vendor type and configuration of a DHCP client
    pub vendor_id: Option<VendorClassIdentifier>,
    pub client_uuid: PxeUuid,
    pub msg_type: DhcpMessageType,
    pub network_interface_version: NetworkInterfaceVersion,
    pub client_identifier: ClientIdentifier,
    pub transaction_id: u32,
    pub secs: u16,
    pub firmware_type: FirmwareType,
}

pub fn pxe_discover(dhcp: DhcpPacket<&[u8]>) -> Result<PxeClientInfo> {
    let mut client_arch: Option<ClientArchType> = None;
    let mut vendor_id: Option<VendorClassIdentifier> = None;
    let mut msg_type: Option<DhcpMessageType> = None;
    let mut network_interface_version: Option<NetworkInterfaceVersion> = None;
    let mut client_uuid: Option<PxeUuid> = None;
    let mut client_identifier: Option<ClientIdentifier> = None;
    let mut firmware_type: FirmwareType = FirmwareType::Unknown;

    if dhcp.opcode() != DhcpMessageType::Request.opcode() {
        return Err(Error::Ignore("Not a dhcp request".to_string()));
    }

    for option in dhcp.options() {
        if let Ok(opt_kind) = SubsetDhcpOption::try_from(option.kind) {
            match opt_kind {
                SubsetDhcpOption::MessageType => {
                    // Message Type
                    let mtype = DhcpMessageType::try_from(option.data[0])
                        .map_err(|e| Error::Malformed(f!("Invalid message type: {}", e)))?;
                    msg_type = Some(mtype);
                }
                SubsetDhcpOption::ClientSystemArchitecture => {
                    let t = ClientArchType::try_from(option.data)?;
                    client_arch = Some(t);
                }
                SubsetDhcpOption::ClientNetworkInterfaceIdentifier => {
                    let t = NetworkInterfaceVersion::try_from(option.data).map_err(|e| {
                        Error::Malformed(f!("Invalid network interface version: {}", e))
                    })?;

                    network_interface_version = Some(t);
                }
                SubsetDhcpOption::ClientUuid => {
                    let t = PxeUuid::try_from(option.data)?;
                    client_uuid = Some(t);
                }
                SubsetDhcpOption::VendorClassIdentifier => {
                    let s = VendorClassIdentifier::try_from(option.data)?;
                    vendor_id = Some(s);
                }
                SubsetDhcpOption::ClientIdentifier => {
                    let t = ClientIdentifier::try_from(option.data)
                        .map_err(|e| Error::Malformed(f!("Invalid client identifier: {}", e)))?;
                    client_identifier = Some(t);
                }
                SubsetDhcpOption::ParameterRequestList | SubsetDhcpOption::MaximumMessageSize => {
                    // Ignore
                }
                SubsetDhcpOption::UserClassInformation => {
                    // iPXE implements this options not adhering to the specification
                    // But we need it to detect iPXE clients

                    match core::str::from_utf8(option.data) {
                        Ok(i) => {
                            if i == "iPXE" {
                                firmware_type = FirmwareType::IPxe;
                            } else {
                                warn!("Unknown firmware type: {}", i);
                            }
                        }
                        Err(_) => warn!("UserClassInformation is not valid utf8"),
                    };
                }
                _ => {
                    warn!("Unhandled PXE option: {:?}", opt_kind)
                }
            }
        }
    }

    // If the client identifier option is not present, use the hardware address from the DHCP packet
    if client_identifier.is_none() {
        let id = ClientIdentifier {
            hardware_type: HardwareType::Ethernet,
            hardware_address: dhcp.client_hardware_address().as_bytes().to_vec(),
        };
        client_identifier = Some(id);
    }

    Ok(PxeClientInfo {
        firmware_type,
        client_arch: client_arch.ok_or(Error::MissingDhcpOption("Client Architecture"))?,
        vendor_id,
        client_identifier: client_identifier
            .ok_or(Error::MissingDhcpOption("Client Identifier"))?,
        client_uuid: client_uuid.ok_or(Error::MissingDhcpOption("Client UUID"))?,
        msg_type: msg_type.ok_or(Error::MissingDhcpOption("Message Type"))?,
        network_interface_version: network_interface_version
            .ok_or(Error::MissingDhcpOption("Network Interface Version"))?,
        transaction_id: dhcp.transaction_id(),
        secs: dhcp.secs(),
    })
}

#[cfg(test)]
mod test {

    use super::*;
    static PXE_DISCOVER: &[u8] = &[
        0x01, 0x01, 0x06, 0x00, 0x43, 0x31, 0xaf, 0x13, 0x00, 0x04, 0x80, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x52, 0x54,
        0x00, 0x12, 0x34, 0x56, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x63, 0x82, 0x53, 0x63,
        0x35, 0x01, 0x01, 0x39, 0x02, 0x05, 0xc0, 0x5d, 0x02, 0x00, 0x00, 0x5e, 0x03, 0x01, 0x02,
        0x01, 0x3c, 0x20, 0x50, 0x58, 0x45, 0x43, 0x6c, 0x69, 0x65, 0x6e, 0x74, 0x3a, 0x41, 0x72,
        0x63, 0x68, 0x3a, 0x30, 0x30, 0x30, 0x30, 0x30, 0x3a, 0x55, 0x4e, 0x44, 0x49, 0x3a, 0x30,
        0x30, 0x32, 0x30, 0x30, 0x31, 0x4d, 0x04, 0x69, 0x50, 0x58, 0x45, 0x37, 0x17, 0x01, 0x03,
        0x06, 0x07, 0x0c, 0x0f, 0x11, 0x1a, 0x2b, 0x3c, 0x42, 0x43, 0x77, 0x80, 0x81, 0x82, 0x83,
        0x84, 0x85, 0x86, 0x87, 0xaf, 0xcb, 0xaf, 0x36, 0xb1, 0x05, 0x01, 0x80, 0x86, 0x10, 0x0e,
        0xeb, 0x03, 0x01, 0x00, 0x00, 0x17, 0x01, 0x01, 0x22, 0x01, 0x01, 0x13, 0x01, 0x01, 0x14,
        0x01, 0x01, 0x11, 0x01, 0x01, 0x27, 0x01, 0x01, 0x19, 0x01, 0x01, 0x19, 0x01, 0x01, 0x10,
        0x01, 0x02, 0x21, 0x01, 0x01, 0x15, 0x01, 0x01, 0x18, 0x01, 0x01, 0x1b, 0x01, 0x01, 0x12,
        0x01, 0x01, 0x3d, 0x07, 0x01, 0x52, 0x54, 0x00, 0x12, 0x34, 0x56, 0x61, 0x11, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0xff,
    ];

    #[test]
    fn test_pxe_request() {
        let dhcp = match DhcpPacket::new_checked(PXE_DISCOVER) {
            Ok(d) => d,
            Err(e) => {
                panic!("Parsing dhcp packet failed: {}", e);
            }
        };
        let info = pxe_discover(dhcp).unwrap();

        assert_eq!(
            info.vendor_id,
            Some(VendorClassIdentifier {
                data: "PXEClient:Arch:00000:UNDI:002001".to_string()
            })
        );
        assert_eq!(info.client_arch, ClientArchType::X86Bios);
        assert_eq!(info.msg_type, DhcpMessageType::Discover);
        assert_eq!(
            info.client_identifier.hardware_address,
            vec![0x52, 0x54, 0x00, 0x12, 0x34, 0x56]
        );
        assert_eq!(info.client_identifier.hardware_type, HardwareType::Ethernet);
        assert_eq!(
            info.client_uuid,
            PxeUuid::try_from([0x00; 17].as_slice()).expect("Failed to create PxeUuid")
        );
    }

    static PXE_OFFER: &[u8] = &[
        0x02, 0x01, 0x06, 0x00, 0x43, 0x31, 0xaf, 0x13, 0x00, 0x04, 0x80, 0x00, 0x00, 0x00, 0x00,
        0x00, 0xc0, 0xa8, 0xb2, 0x4f, 0xc0, 0xa8, 0xb2, 0x01, 0x00, 0x00, 0x00, 0x00, 0x52, 0x54,
        0x00, 0x12, 0x34, 0x56, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x63, 0x82, 0x53, 0x63,
        0x35, 0x01, 0x02, 0x36, 0x04, 0xc0, 0xa8, 0xb2, 0x01, 0x33, 0x04, 0x00, 0x0d, 0x2f, 0x00,
        0x3a, 0x04, 0x00, 0x06, 0x97, 0x80, 0x3b, 0x04, 0x00, 0x0b, 0x89, 0x20, 0x01, 0x04, 0xff,
        0xff, 0xff, 0x00, 0x03, 0x04, 0xc0, 0xa8, 0xb2, 0x01, 0x06, 0x04, 0xc0, 0xa8, 0xb2, 0x01,
        0x0f, 0x09, 0x66, 0x72, 0x69, 0x74, 0x7a, 0x2e, 0x62, 0x6f, 0x78, 0xff, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];

    #[test]
    fn test_pxe_offer() {
        let dhcp = match DhcpPacket::new_checked(PXE_OFFER) {
            Ok(d) => d,
            Err(e) => {
                panic!("Parsing dhcp packet failed: {}", e);
            }
        };
        let info = pxe_discover(dhcp);

        match info {
            Ok(_) => panic!("PXE offer should not be parsed as a PXE discover packet"),
            Err(Error::Ignore(_)) => (),
            _ => panic!("This should not happen"),
        }
    }
}
