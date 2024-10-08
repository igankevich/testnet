use std::ffi::c_int;
use std::ffi::CString;
use std::fs::File;
use std::net::Ipv4Addr;
use std::os::fd::AsRawFd;
use std::os::fd::FromRawFd;
use std::os::fd::OwnedFd;
use std::os::fd::RawFd;
use std::path::Path;

use ipnet::IpNet;
use mio_pidfd::PidFd;
use nix::mount::mount;
use nix::mount::MsFlags;
use nix::sched::setns;
use nix::sched::CloneFlags;
use nix::sys::prctl::set_name;
use nix::sys::socket::SockProtocol;
use nix::sys::wait::WaitStatus;
use nix::unistd::dup2;
use nix::unistd::pipe;
use nix::unistd::sethostname;
use nix::unistd::Gid;
use nix::unistd::Pid;
use nix::unistd::Uid;
use tempfile::TempDir;

use crate::log_format;
use crate::pipe_channel;
use crate::CallbackResult;
use crate::Context;
use crate::IpcClient;
use crate::IpcServer;
use crate::NetConfig;
use crate::Netlink;
use crate::NodeConfig;
use crate::PipeReceiver;
use crate::Process;

/// Virtual network.
///
/// This struct offers more granular control over the network compared to `testnet` function.
/// See `testnet` for more details.
pub struct Network {
    main: Process,
}

impl Network {
    /// Create new virtual network with the specified configuration.
    ///
    /// Launches child processes in their own network namespaces.
    /// See `testnet` for more details.
    pub fn new<C: Into<NodeConfig>, F: FnOnce(Context) -> CallbackResult + Clone>(
        config: NetConfig<C, F>,
    ) -> Result<Self, std::io::Error> {
        let (sender, receiver) = pipe_channel()?;
        let main = Process::spawn(
            || network_switch_main(receiver.into(), config),
            STACK_SIZE,
            CloneFlags::CLONE_NEWNET
                | CloneFlags::CLONE_NEWUSER
                | CloneFlags::CLONE_NEWUTS
                | CloneFlags::CLONE_NEWNS,
        )?;
        // update uid map
        std::fs::write(
            format!("/proc/{}/uid_map", main.id()),
            format!("0 {} 1", Uid::current()),
        )?;
        // setgroups deny
        std::fs::write(format!("/proc/{}/setgroups", main.id()), "deny")?;
        // update gid map
        std::fs::write(
            format!("/proc/{}/gid_map", main.id()),
            format!("0 {} 1", Gid::current()),
        )?;
        // notify the child process
        sender.close()?;
        Ok(Self { main })
    }

    /// Wait until the child processes exit successfully or one of the node processes fails.
    pub fn wait(&self) -> Result<WaitStatus, std::io::Error> {
        Ok(self.main.wait()?)
    }
}

/// Main entry point to the library.
///
/// Launches virtual network using Linux network namespaces
/// and runs specified `main` function in each node's process.
/// If a node process exits with non-zero value, the test fails.
/// If all node processes exit with zero values, the test succeeds.
///
/// This function internally launches child process in its own network namespace,
/// and this process in turn launches another child process for each network node
/// (again in its own network namespace).
/// Nodes do not have access to the outside network.
pub fn testnet<C: Into<NodeConfig>, F: FnOnce(Context) -> CallbackResult + Clone>(
    config: NetConfig<C, F>,
) -> Result<(), std::io::Error> {
    let network = Network::new(config)?;
    match network.wait()? {
        WaitStatus::Exited(_, 0) => Ok(()),
        _ => Err(std::io::Error::other("some nodes failed")),
    }
}

fn network_switch_main<C: Into<NodeConfig>, F: FnOnce(Context) -> CallbackResult + Clone>(
    receiver: PipeReceiver,
    config: NetConfig<C, F>,
) -> c_int {
    match do_network_switch_main(receiver, config) {
        Ok(_) => 0,
        Err(e) => {
            log_format!("network main failed: {}", e);
            1
        }
    }
}

fn do_network_switch_main<C: Into<NodeConfig>, F: FnOnce(Context) -> CallbackResult + Clone>(
    receiver: PipeReceiver,
    config: NetConfig<C, F>,
) -> CallbackResult {
    set_process_name(SWITCH_NAME)?;
    sethostname(SWITCH_NAME)?;
    // wait for uid/gid mappings to be done by the parent process
    receiver.wait_until_closed()?;
    let mut netlink = Netlink::new(SockProtocol::NetlinkRoute)?;
    netlink.new_bridge(BRIDGE_IFNAME)?;
    let mut nodes: Vec<Process> = Vec::with_capacity(config.nodes.len());
    let net = IpNet::new(Ipv4Addr::new(10, 84, 0, 0).into(), 16)?;
    let mut all_node_configs = Vec::with_capacity(config.nodes.len());
    for (i, node_config) in config.nodes.into_iter().enumerate() {
        let mut node_config: NodeConfig = node_config.into();
        if node_config.name.is_empty() {
            node_config.name = outer_ifname(i);
        }
        if node_config.ifaddr.addr().is_unspecified() {
            node_config.ifaddr = IpNet::new(
                net.hosts()
                    .nth(i)
                    .ok_or("exhausted available IP adddress range")?,
                net.prefix_len(),
            )?;
        }
        all_node_configs.push(node_config);
    }
    let workdir = TempDir::new()?;
    let hosts = workdir.path().join("hosts");
    std::fs::write(
        hosts.as_path(),
        all_node_configs
            .iter()
            .fold(String::with_capacity(4096), |mut buf, node| {
                use std::fmt::Write;
                let _ = writeln!(&mut buf, "{} {}", node.ifaddr.addr(), node.name);
                buf
            }),
    )?;
    if let Err(e) = mount(
        Some(hosts.as_path()),
        "/etc/hosts",
        None::<&Path>,
        MsFlags::MS_BIND,
        None::<&Path>,
    ) {
        log_format!(
            "WARNING: bind mount failed ({}), node hostnames will not be available",
            e
        );
    }
    // TODO fall back on nss modules??? still will not work for musl
    let mut ipc_fds: Vec<(OwnedFd, OwnedFd, PidFd, OwnedFd, String)> =
        Vec::with_capacity(all_node_configs.len());
    for i in 0..all_node_configs.len() {
        let (in_self, out_other) = pipe()?;
        let (in_other, out_self) = pipe()?;
        let (output_self, output_other) = pipe()?;
        let in_self_fd = in_self.as_raw_fd();
        let in_other_fd = in_other.as_raw_fd();
        let out_self_fd = out_self.as_raw_fd();
        let out_other_fd = out_other.as_raw_fd();
        let output_other_fd = output_other.as_raw_fd();
        let output_self_fd = output_self.as_raw_fd();
        let main = config.main.clone();
        let node_name = all_node_configs[i].name.clone();
        let all_node_configs = all_node_configs.clone();
        let process = Process::spawn(
            || {
                // drop unused pipe ends
                unsafe {
                    OwnedFd::from_raw_fd(in_self_fd);
                    OwnedFd::from_raw_fd(out_self_fd);
                    OwnedFd::from_raw_fd(output_self_fd);
                }
                network_node_main(
                    in_other_fd,
                    out_other_fd,
                    output_other_fd,
                    i,
                    main,
                    all_node_configs,
                )
            },
            STACK_SIZE,
            CloneFlags::CLONE_NEWNET | CloneFlags::CLONE_NEWUTS,
        )?;
        // drop unused pipe ends
        drop(in_other);
        drop(out_other);
        drop(output_other);
        let pid_fd = process.fd()?;
        ipc_fds.push((in_self, out_self, pid_fd, output_self, node_name));
        nodes.push(process);
    }
    let mut ipc_server = IpcServer::new(ipc_fds)?;
    ipc_server.run()?;
    let mut all_ret = Vec::with_capacity(nodes.len());
    for node in nodes.into_iter() {
        let status = node.wait()?;
        all_ret.push(status);
    }
    if all_ret.iter().all(wait_status_ok) {
        Ok(())
    } else {
        use std::fmt::Write;
        let mut buf = String::with_capacity(4096);
        writeln!(&mut buf, "some nodes failed:")?;
        for (i, status) in all_ret.into_iter().enumerate() {
            writeln!(
                &mut buf,
                "- node {} exited with {}",
                i,
                wait_status_to_string(status)
            )?;
        }
        Err(buf.into())
    }
}

fn network_node_main<F: FnOnce(Context) -> CallbackResult>(
    ipc_in_fd: RawFd,
    ipc_out_fd: RawFd,
    output_fd: RawFd,
    i: usize,
    main: F,
    node_config: Vec<NodeConfig>,
) -> c_int {
    match do_network_node_main(ipc_in_fd, ipc_out_fd, output_fd, i, main, node_config) {
        Ok(_) => 0,
        Err(e) => {
            log_format!("child `main` failed: {}", e);
            1
        }
    }
}

fn do_network_node_main<F: FnOnce(Context) -> CallbackResult>(
    ipc_in_fd: RawFd,
    ipc_out_fd: RawFd,
    output_fd: RawFd,
    i: usize,
    main: F,
    nodes: Vec<NodeConfig>,
) -> CallbackResult {
    // redirect stdout/stderr
    dup2(output_fd, 1)?;
    dup2(output_fd, 2)?;
    // clonse stdin
    nix::unistd::close(0)?;
    set_process_name(&nodes[i].name)?;
    sethostname(&nodes[i].name)?;
    configure_network(i, nodes[i].ifaddr)?;
    let ipc_in_fd = unsafe { OwnedFd::from_raw_fd(ipc_in_fd) };
    let ipc_out_fd = unsafe { OwnedFd::from_raw_fd(ipc_out_fd) };
    let context = Context {
        node_index: i,
        nodes,
        ipc_client: IpcClient::new(ipc_in_fd, ipc_out_fd),
        step_name: None,
        step: 0,
        ifname: inner_ifname(i),
    };
    main(context).map_err(|e| format!("node `main` failed: {}", e).into())
}

fn configure_network(i: usize, ifaddr: IpNet) -> Result<(), std::io::Error> {
    let old_ns_file = File::open(format!("/proc/{}/ns/net", Pid::this()))?;
    let parent_ns_file = File::open(format!("/proc/{}/ns/net", Pid::parent()))?;
    // go back to parent's network namespace
    setns(parent_ns_file, CloneFlags::CLONE_NEWNET)?;
    let mut netlink = Netlink::new(SockProtocol::NetlinkRoute)?;
    let bridge_index = netlink.index(BRIDGE_IFNAME)?;
    let inner = inner_ifname(i);
    let outer = outer_ifname(i);
    netlink.new_veth_pair(outer.clone(), inner.clone())?;
    netlink.set_up(outer.clone())?;
    netlink.set_bridge(outer.clone(), bridge_index)?;
    netlink.set_network_namespace(inner.clone(), old_ns_file.as_raw_fd())?;
    drop(netlink);
    // go back to child's network namespace
    setns(old_ns_file, CloneFlags::CLONE_NEWNET)?;
    // we need new netlink socket because we changed ns
    let mut netlink = Netlink::new(SockProtocol::NetlinkRoute)?;
    netlink.set_up(LOOPBACK_IFNAME)?;
    let inner_index = netlink.index(inner.clone())?;
    netlink.set_up(inner)?;
    netlink.set_ifaddr(inner_index, ifaddr)?;
    drop(netlink);
    Ok(())
}

fn wait_status_ok(status: &WaitStatus) -> bool {
    matches!(status, WaitStatus::Exited(_, code) if code == &0)
}

fn wait_status_to_string(status: WaitStatus) -> String {
    match status {
        WaitStatus::Exited(_, code) => format!("code {}", code),
        WaitStatus::Signaled(_, signal, _) => format!("signal {:?}", signal),
        _ => "unknown".to_string(),
    }
}

fn outer_ifname(i: usize) -> String {
    format!("n{}", i)
}

fn inner_ifname(i: usize) -> String {
    format!("veth{}", i)
}

fn set_process_name(name: &str) -> Result<(), std::io::Error> {
    let name = format!("testnet/{}", name);
    let c_string = CString::new(name)?;
    Ok(set_name(c_string.as_c_str())?)
}

const STACK_SIZE: usize = 4096 * 16;
const BRIDGE_IFNAME: &str = "testnet";
const SWITCH_NAME: &str = "switch";
const LOOPBACK_IFNAME: &str = "lo";
