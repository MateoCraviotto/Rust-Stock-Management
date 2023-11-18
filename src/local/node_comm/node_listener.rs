use std::{net::Ipv4Addr, sync::Arc};

use serde::{de::DeserializeOwned, Serialize};
use tokio::{net::TcpListener, select, task::JoinHandle};
use tokio_util::sync::CancellationToken;

use crate::info;

use super::{node_communication::NodeCommunication, protocol::Operator, NodeID};

struct NodeListener<T: Send + Serialize + DeserializeOwned + 'static, O: Operator<T> + 'static> {
    cancel: CancellationToken,
    node_comm: Arc<NodeCommunication<T, O>>,
    task_handle: JoinHandle<anyhow::Result<()>>,
    me: NodeID,
}

impl<T: Send + Serialize + DeserializeOwned + 'static, O: Operator<T>> NodeListener<T, O> {
    pub fn start(port: u16, ip: Ipv4Addr, me: NodeID, operator: O) -> Self {
        let cancel = CancellationToken::new();
        let node_comm = NodeCommunication::start(cancel.clone(), me, operator);
        let handle = tokio::spawn(Self::run(port, ip, cancel.clone(), node_comm.clone()));

        Self {
            cancel,
            task_handle: handle,
            node_comm,
            me,
        }
    }

    async fn run(
        port: u16,
        ip: Ipv4Addr,
        cancel: CancellationToken,
        node_comm: Arc<NodeCommunication<T, O>>,
    ) -> anyhow::Result<()> {
        //TODO: conectar a todos los demas que nos pasen
        let listener = TcpListener::bind(format!("{}:{}", &ip, port)).await?;

        let mut node_comms = vec![];

        'accept: loop {
            select! {
                conn_result = listener.accept() => {
                    let (stream, addr) = conn_result?;
                    info!(format!("New node connection for {}", addr));
                    node_comms.push(node_comm.new_node(stream));
                }

                _ = cancel.cancelled() => {
                    info!("Got order to take down the node TCP stream");
                    break 'accept;
                }
            }
        }

        let _ = futures::future::join_all(node_comms.into_iter().map(|nc| nc.shutdown()));

        Ok(())
    }

    pub fn shutdown(&self) {
        self.cancel.cancel();
    }

    async fn commmunicate(&self, node: NodeID, msg: T) -> anyhow::Result<T> {
        self.node_comm.commmunicate(node, msg).await
    }

    async fn send_message(&self, node: NodeID, msg: T) -> anyhow::Result<()> {
        self.node_comm.send_message(node, msg).await
    }
}
