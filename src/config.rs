use ipnet::IpNet;

use crate::Context;

/// Result of the node's `main` function.
pub type CallbackResult = Result<(), Box<dyn std::error::Error>>;

/// Network configuration.
///
/// This includes the `main` function that is executed on each node
/// and configuration of all the nodes.
pub struct NetConfig<F: FnOnce(Context) -> CallbackResult> {
    pub nodes: Vec<NodeConfig>,
    pub main: F,
}

#[derive(Default, Clone)]
pub struct NodeConfig {
    /// Host name.
    pub name: String,
    /// Network interface address.
    pub ifaddr: IpNet,
}
