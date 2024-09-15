use ipnet::IpNet;

use crate::Context;

/// Result of the node's `main` function.
pub type CallbackResult = Result<(), Box<dyn std::error::Error>>;

/// Network configuration.
///
/// This includes the `main` function that is executed on each node
/// and configuration of all the nodes.
pub struct NetConfig<C: Into<NodeConfig>, F: FnOnce(Context) -> CallbackResult> {
    /// Nodes' configurations.
    pub nodes: Vec<C>,
    /// Closure that is run on each node.
    pub main: F,
}

/// Node configuration.
#[derive(Default, Clone)]
pub struct NodeConfig {
    /// Host name.
    pub name: String,
    /// Network interface address.
    pub ifaddr: IpNet,
}

impl From<String> for NodeConfig {
    fn from(name: String) -> Self {
        Self {
            name,
            ..Default::default()
        }
    }
}

impl From<&str> for NodeConfig {
    fn from(name: &str) -> Self {
        Self {
            name: name.into(),
            ..Default::default()
        }
    }
}
