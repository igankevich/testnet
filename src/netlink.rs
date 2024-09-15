use std::fmt::Debug;
use std::os::fd::AsRawFd;
use std::os::fd::OwnedFd;

use ipnet::IpNet;
use netlink_packet_core::NetlinkDeserializable;
use netlink_packet_core::NetlinkMessage;
use netlink_packet_core::NetlinkPayload;
use netlink_packet_core::NetlinkSerializable;
use netlink_packet_core::NLM_F_ACK;
use netlink_packet_core::NLM_F_CREATE;
use netlink_packet_core::NLM_F_EXCL;
use netlink_packet_core::NLM_F_REQUEST;
use netlink_packet_route::address::AddressAttribute;
use netlink_packet_route::address::AddressMessage;
use netlink_packet_route::link::InfoData;
use netlink_packet_route::link::InfoKind;
use netlink_packet_route::link::InfoVeth;
use netlink_packet_route::link::LinkAttribute;
use netlink_packet_route::link::LinkFlags;
use netlink_packet_route::link::LinkInfo;
use netlink_packet_route::link::LinkMessage;
use netlink_packet_route::RouteNetlinkMessage;
use nix::sys::socket::socket;
use nix::sys::socket::AddressFamily;
use nix::sys::socket::SockFlag;
use nix::sys::socket::SockProtocol;
use nix::sys::socket::SockType;
use nix::unistd::Pid;

pub(crate) struct Netlink {
    socket: OwnedFd,
}

impl Netlink {
    pub(crate) fn new(protocol: SockProtocol) -> Result<Self, std::io::Error> {
        let socket = socket(
            AddressFamily::Netlink,
            SockType::Raw,
            SockFlag::SOCK_CLOEXEC,
            protocol,
        )?;
        Ok(Self { socket })
    }

    pub(crate) fn new_veth_pair(
        &mut self,
        name: impl ToString,
        peer_name: impl ToString,
    ) -> Result<(), std::io::Error> {
        let mut peer = LinkMessage::default();
        peer.attributes
            .push(LinkAttribute::IfName(name.to_string()));
        let link_info_data = InfoData::Veth(InfoVeth::Peer(peer));
        let mut link = LinkMessage::default();
        link.attributes
            .push(LinkAttribute::IfName(peer_name.to_string()));
        link.attributes.push(LinkAttribute::LinkInfo(vec![
            LinkInfo::Kind(InfoKind::Veth),
            LinkInfo::Data(link_info_data),
        ]));
        link.header.flags.insert(LinkFlags::Up);
        link.header.change_mask.insert(LinkFlags::Up);
        let mut message = NetlinkMessage::from(RouteNetlinkMessage::NewLink(link));
        message.header.flags = NLM_F_REQUEST | NLM_F_ACK | NLM_F_EXCL | NLM_F_CREATE;
        message.finalize();
        let message = self.send(&message)?;
        check_ok(message)?;
        Ok(())
    }

    pub(crate) fn new_bridge(&mut self, name: impl ToString) -> Result<(), std::io::Error> {
        let mut link = LinkMessage::default();
        link.attributes
            .push(LinkAttribute::IfName(name.to_string()));
        link.attributes
            .push(LinkAttribute::LinkInfo(vec![LinkInfo::Kind(
                InfoKind::Bridge,
            )]));
        link.header.flags.insert(LinkFlags::Up);
        link.header.change_mask.insert(LinkFlags::Up);
        let mut message = NetlinkMessage::from(RouteNetlinkMessage::NewLink(link));
        message.header.flags = NLM_F_REQUEST | NLM_F_ACK | NLM_F_EXCL | NLM_F_CREATE;
        message.finalize();
        let message = self.send(&message)?;
        check_ok(message)?;
        Ok(())
    }

    pub(crate) fn set_up(&mut self, name: impl ToString) -> Result<(), std::io::Error> {
        let mut link = LinkMessage::default();
        link.attributes
            .push(LinkAttribute::IfName(name.to_string()));
        link.header.flags.insert(LinkFlags::Up);
        link.header.change_mask.insert(LinkFlags::Up);
        let mut message = NetlinkMessage::from(RouteNetlinkMessage::SetLink(link));
        message.header.flags = NLM_F_REQUEST | NLM_F_ACK;
        message.finalize();
        let message = self.send(&message)?;
        check_ok(message)?;
        Ok(())
    }

    pub(crate) fn set_bridge(
        &mut self,
        name: String,
        bridge_index: u32,
    ) -> Result<(), std::io::Error> {
        let mut link = LinkMessage::default();
        link.attributes.push(LinkAttribute::IfName(name));
        link.attributes
            .push(LinkAttribute::Controller(bridge_index));
        let mut message = NetlinkMessage::from(RouteNetlinkMessage::SetLink(link));
        message.header.flags = NLM_F_REQUEST | NLM_F_ACK;
        message.finalize();
        let message = self.send(&message)?;
        check_ok(message)?;
        Ok(())
    }

    pub(crate) fn set_network_namespace(
        &mut self,
        name: impl ToString,
        pid: Pid,
    ) -> Result<(), std::io::Error> {
        let mut link = LinkMessage::default();
        link.attributes
            .push(LinkAttribute::IfName(name.to_string()));
        link.attributes
            .push(LinkAttribute::NetNsPid(pid.as_raw() as u32));
        let mut message = NetlinkMessage::from(RouteNetlinkMessage::SetLink(link));
        message.header.flags = NLM_F_REQUEST | NLM_F_ACK;
        message.finalize();
        let message = self.send(&message)?;
        check_ok(message)?;
        Ok(())
    }

    pub(crate) fn set_ifaddr(&mut self, index: u32, ifaddr: IpNet) -> Result<(), std::io::Error> {
        use netlink_packet_route::AddressFamily;
        let mut message = AddressMessage::default();
        message.header.prefix_len = ifaddr.prefix_len();
        message.header.index = index;
        message.header.family = match ifaddr {
            IpNet::V4(_) => AddressFamily::Inet,
            IpNet::V6(_) => AddressFamily::Inet6,
        };
        message
            .attributes
            .push(AddressAttribute::Address(ifaddr.addr()));
        message
            .attributes
            .push(AddressAttribute::Local(ifaddr.addr()));
        let mut message = NetlinkMessage::from(RouteNetlinkMessage::NewAddress(message));
        message.header.flags = NLM_F_REQUEST | NLM_F_ACK | NLM_F_EXCL | NLM_F_CREATE;
        message.finalize();
        let message = self.send(&message)?;
        check_ok(message)?;
        Ok(())
    }

    pub(crate) fn index(&mut self, name: impl ToString) -> Result<u32, std::io::Error> {
        let mut link = LinkMessage::default();
        link.attributes
            .push(LinkAttribute::IfName(name.to_string()));
        let mut message = NetlinkMessage::from(RouteNetlinkMessage::GetLink(link));
        message.header.flags = NLM_F_REQUEST | NLM_F_ACK;
        message.finalize();
        let message = self.send(&message)?;
        let index = match message.payload {
            NetlinkPayload::InnerMessage(RouteNetlinkMessage::NewLink(ref inner)) => {
                Some(inner.header.index)
            }
            _ => None,
        };
        match index {
            Some(index) => Ok(index),
            None => Err(std::io::Error::other(format!(
                "netlink returned unexpected data: {:?}",
                message,
            ))),
        }
    }

    fn send<I: NetlinkSerializable + NetlinkDeserializable + Debug>(
        &mut self,
        message: &NetlinkMessage<I>,
    ) -> Result<NetlinkMessage<I>, std::io::Error> {
        let mut buf = vec![0_u8; message.header.length as usize];
        // Serialize the packet
        message.serialize(&mut buf[..]);
        let n = nix::unistd::write(&mut self.socket, &buf)?;
        if n != buf.len() {
            return Err(std::io::Error::other("partial write"));
        }
        buf.clear();
        buf.resize(4096, 0_u8);
        let n = nix::unistd::read(self.socket.as_raw_fd(), &mut buf)?;
        buf.truncate(n);
        let message = NetlinkMessage::<I>::deserialize(&buf)
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        Ok(message)
    }
}

fn check_ok<I: Debug>(message: NetlinkMessage<I>) -> Result<NetlinkMessage<I>, std::io::Error> {
    match message.payload {
        NetlinkPayload::Error(ref error) => {
            if let Some(code) = error.code {
                return Err(std::io::Error::other(format!(
                    "netlink failed with error code {}",
                    code
                )));
            }
        }
        other => {
            return Err(std::io::Error::other(format!(
                "netlink returned unexpected data: {:?}",
                other,
            )));
        }
    }
    Ok(message)
}
