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
    /// By default step name equals its sequential number.
    /// This function overrides the default.
    pub fn step(&mut self, name: impl Display) {
        self.step_name = Some(format!("\"{name}\""));
    }

    /// Participate in global synchronization.
    ///
    /// This function sends the specified `data` to any node
    /// that calls `recv` in the current step.
    /// Only one node per step can send the data, others should either call `recv` or `wait.
    /// This function marks the the end of the current step.
    pub fn send(&mut self, data: Vec<u8>) -> Result<(), std::io::Error> {
        self.next_step();
        self.ipc_client.send(&IpcMessage::Send(data))?;
        self.ipc_client.flush()?;
        self.ipc_client.fill_buf()?;
        let response = self
            .ipc_client
            .recv()?
            .ok_or_else(|| std::io::Error::other("no response"))?;
        if !matches!(response, IpcMessage::Wait) {
            return Err(std::io::Error::other("invalid response"));
        }
        self.print_step();
        Ok(())
    }

    /// Convenience wrapper around `send` that sends a string instead of arbitrary data.
    pub fn send_text(&mut self, text: String) -> Result<(), std::io::Error> {
        self.send(text.into())
    }

    /// Participate in global synchronization.
    ///
    /// This function receives whatever data was sent by some node in the current step.
    /// Only one node per step can send the data, others should either call `recv` or `wait.
    /// This function marks the the end of the current step.
    pub fn recv(&mut self) -> Result<Vec<u8>, std::io::Error> {
        self.ipc_client.send(&IpcMessage::Receive)?;
        self.ipc_client.flush()?;
        self.ipc_client.fill_buf()?;
        let response = self
            .ipc_client
            .recv()?
            .ok_or_else(|| std::io::Error::other("no response"))?;
        match response {
            IpcMessage::Send(data) => Ok(data),
            _ => Err(std::io::Error::other("invalid response")),
        }
    }

    /// Convenience wrapper around `recv` that receives a string instead of arbitrary data.
    pub fn recv_text(&mut self) -> Result<String, std::io::Error> {
        let data = self.recv()?;
        let text = String::from_utf8(data).map_err(std::io::Error::other)?;
        Ok(text)
    }

    /// Participate in global synchronization.
    ///
    /// This function waits until the current step completes without sending or receiving any data.
    /// Only one node per step can send the data, others should either call `recv` or `wait.
    pub fn wait(&mut self) -> Result<(), std::io::Error> {
        self.ipc_client.send(&IpcMessage::Wait)?;
        self.ipc_client.flush()?;
        self.ipc_client.fill_buf()?;
        let response = self
            .ipc_client
            .recv()?
            .ok_or_else(|| std::io::Error::other("no response"))?;
        if !matches!(response, IpcMessage::Wait) {
            return Err(std::io::Error::other("invalid response"));
        }
        Ok(())
    }

    fn next_step(&mut self) {
        self.step += 1;
        if self.step_name.is_none() {
            self.step_name = Some(self.step.to_string());
        }
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
