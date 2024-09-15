use std::fmt::Display;

use crate::log_format;
use crate::IpcClient;
use crate::IpcMessage;
use crate::NodeConfig;

/// Node execution context.
///
/// Every node's `main` function receives its own instance of this context.
pub struct Context {
    pub(crate) node_index: usize,
    pub(crate) nodes: Vec<NodeConfig>,
    pub(crate) ipc_client: IpcClient,
    pub(crate) step_name: Option<String>,
    pub(crate) step: usize,
    pub(crate) ifname: String,
}

impl Context {
    /// Current network node index.
    pub fn current_node_index(&self) -> usize {
        self.node_index
    }

    /// Current network node name (which is also a hostname).
    pub fn current_node_name(&self) -> &str {
        self.nodes[self.node_index].name.as_str()
    }

    /// Current node network interface name.
    pub fn current_node_ifname(&self) -> &str {
        &self.ifname
    }

    /// Current node configuration.
    pub fn current_node(&self) -> &NodeConfig {
        &self.nodes[self.node_index]
    }

    /// Configuration of all the nodes in the network.
    pub fn nodes(&self) -> &[NodeConfig] {
        &self.nodes
    }

    /// Name the current step.
    ///
    /// Testnet will output the name of the step on failure.
    pub fn step(&mut self, name: impl Display) {
        self.step_name = Some(format!("\"{name}\""));
    }

    /// Broadcast from one node to every other one.
    pub fn broadcast_one(&mut self) -> BroadcastOne {
        BroadcastOne { context: self }
    }

    /// Broadcast data from each node to each node.
    ///
    /// The data is received by each node in a vec where each index corresponds to the node index.
    pub fn broadcast_all(&mut self, data: Vec<u8>) -> Result<Vec<Vec<u8>>, std::io::Error> {
        self.next_step();
        self.ipc_client.send(&IpcMessage::BroadcastAllSend(data))?;
        self.ipc_client.flush()?;
        self.ipc_client.fill_buf()?;
        let response = self
            .ipc_client
            .recv()?
            .ok_or_else(|| std::io::Error::other("no response"))?;
        let all_data = match response {
            IpcMessage::BroadcastAllRecv(payload) => payload,
            _ => return Err(std::io::Error::other("invalid response")),
        };
        self.print_step();
        Ok(all_data)
    }

    /// Convenience wrapper around `broadcast_all` that receives a string instead of arbitrary data.
    pub fn broadcast_all_string(&mut self, data: String) -> Result<Vec<String>, std::io::Error> {
        self.broadcast_all(data.into())?
            .into_iter()
            .map(|data| String::from_utf8(data))
            .collect::<Result<Vec<String>, _>>()
            .map_err(std::io::Error::other)
    }

    fn next_step(&mut self) {
        self.step += 1;
    }

    fn print_step(&mut self) {
        if let Some(step) = self.step_name.take() {
            log_format!("step {}: ok", step);
        }
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        if let Some(step) = self.step_name.take() {
            log_format!("step {}: failed", step);
        }
    }
}

/// Broadcast from one node to every other one.
pub struct BroadcastOne<'a> {
    context: &'a mut Context,
}

impl<'a> BroadcastOne<'a> {
    /// Send the data by the initiator.
    ///
    /// This function sends the specified `data` to any node
    /// that calls `recv` in the current step.
    /// Only one node per step can send the data, others should either call `recv` or `wait`.
    /// This function marks the the end of the current step.
    pub fn send(self, data: Vec<u8>) -> Result<(), std::io::Error> {
        self.context.next_step();
        self.context.ipc_client.send(&IpcMessage::Send(data))?;
        self.context.ipc_client.flush()?;
        self.context.ipc_client.fill_buf()?;
        let response = self
            .context
            .ipc_client
            .recv()?
            .ok_or_else(|| std::io::Error::other("no response"))?;
        if !matches!(response, IpcMessage::Wait) {
            return Err(std::io::Error::other("invalid response"));
        }
        self.context.print_step();
        Ok(())
    }

    /// Convenience wrapper around `send` that sends a string instead of arbitrary data.
    pub fn send_string(self, data: String) -> Result<(), std::io::Error> {
        self.send(data.into())
    }

    /// Receive the data from the initiator.
    ///
    /// This function receives whatever data was sent by some node in the current step.
    /// Only one node per step can send the data, others should either call `recv` or `wait.
    /// This function marks the the end of the current step.
    pub fn recv(&mut self) -> Result<Vec<u8>, std::io::Error> {
        self.context.ipc_client.send(&IpcMessage::Receive)?;
        self.context.ipc_client.flush()?;
        self.context.ipc_client.fill_buf()?;
        let response = self
            .context
            .ipc_client
            .recv()?
            .ok_or_else(|| std::io::Error::other("no response"))?;
        match response {
            IpcMessage::Send(data) => Ok(data),
            _ => Err(std::io::Error::other("invalid response")),
        }
    }

    /// Convenience wrapper around `recv` that receives a string instead of arbitrary data.
    pub fn recv_string(mut self) -> Result<String, std::io::Error> {
        let data = self.recv()?;
        String::from_utf8(data).map_err(std::io::Error::other)
    }

    /// Wait until broadcast is finished without sending or receiving any data.
    ///
    /// This function waits until the current step completes without sending or receiving any data.
    /// Only one node per step can send the data, others should either call `recv` or `wait.
    pub fn wait(self) -> Result<(), std::io::Error> {
        self.context.ipc_client.send(&IpcMessage::Wait)?;
        self.context.ipc_client.flush()?;
        self.context.ipc_client.fill_buf()?;
        let response = self
            .context
            .ipc_client
            .recv()?
            .ok_or_else(|| std::io::Error::other("no response"))?;
        if !matches!(response, IpcMessage::Wait) {
            return Err(std::io::Error::other("invalid response"));
        }
        Ok(())
    }
}
