use std::collections::HashMap;

use mio::Poll;
use mio::Token;

use crate::format_error;
use crate::BroadcastPayload;
use crate::IpcClient;
use crate::IpcMessage;

pub(crate) struct IpcStateMachine {
    num_nodes: usize,
    /// Node index of the initiator.
    broadcast_initiator: Option<usize>,
    broadcasts: HashMap<usize, Broadcast>,
}

impl IpcStateMachine {
    pub(crate) fn new(num_nodes: usize) -> Self {
        Self {
            num_nodes,
            broadcast_initiator: None,
            broadcasts: Default::default(),
        }
    }

    pub(crate) fn on_message(
        &mut self,
        message: IpcMessage,
        from_node_index: usize,
        clients: &mut [IpcClient],
        writer_token: Token,
        poll: &mut Poll,
    ) -> Result<(), std::io::Error> {
        match message {
            IpcMessage::Send(payload) => {
                if let Some(i) = self.broadcast_initiator {
                    return Err(format_error!(
                        "another broadcast from node `{}` is in progress",
                        i
                    ));
                }
                self.insert_broadcast(from_node_index, Broadcast::Send(payload))?;
                self.broadcast_initiator = Some(from_node_index);
            }
            IpcMessage::Receive => {
                self.insert_broadcast(from_node_index, Broadcast::Receive)?;
            }
            IpcMessage::Wait => {
                self.insert_broadcast(from_node_index, Broadcast::Wait)?;
            }
        }
        if self.broadcasts.len() == self.num_nodes {
            self.finalize_broadcast(clients, writer_token, poll)?;
        }
        Ok(())
    }

    fn insert_broadcast(&mut self, i: usize, broadcast: Broadcast) -> Result<(), std::io::Error> {
        use std::collections::hash_map::Entry;
        match self.broadcasts.entry(i) {
            Entry::Vacant(v) => {
                v.insert(broadcast);
                Ok(())
            }
            Entry::Occupied(_) => Err(std::io::Error::other(
                "only one message per broadcast is permitted",
            )),
        }
    }

    fn finalize_broadcast(
        &mut self,
        clients: &mut [IpcClient],
        writer_token: Token,
        poll: &mut Poll,
    ) -> Result<(), std::io::Error> {
        let initiator = match self.broadcast_initiator {
            Some(initiator) => initiator,
            None => return Err(std::io::Error::other("broadcast initiator is missing")),
        };
        // replace Broadcast::Payload with Broadcast::Wait
        let payload = self
            .broadcasts
            .insert(initiator, Broadcast::Wait)
            .ok_or_else(|| std::io::Error::other("broadcast payload is missing"))?;
        let payload = match payload {
            Broadcast::Send(data) => data,
            _ => return Err(std::io::Error::other("initiator sent wrong message")),
        };
        for (i, broadcast) in self.broadcasts.drain() {
            let message = match broadcast {
                Broadcast::Receive => IpcMessage::Send(payload.clone()),
                Broadcast::Wait => IpcMessage::Wait,
                _ => continue,
            };
            clients[i].send(&message)?;
            clients[i].send_finalize(writer_token, poll)?;
        }
        self.broadcasts.clear();
        self.broadcast_initiator = None;
        Ok(())
    }
}

#[derive(Clone)]
enum Broadcast {
    Send(BroadcastPayload),
    Receive,
    Wait,
}
