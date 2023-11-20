use actix::{
    dev::{MessageResponse, ToEnvelope},
    Actor, Addr, Handler, Message, ResponseActFuture,
};
use anyhow::bail;
use core::fmt::Debug;
use serde::{de::DeserializeOwned, Serialize};
use std::{net::Ipv4Addr, sync::Arc};
use tokio::{
    net::{TcpListener, TcpStream, ToSocketAddrs},
    select,
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;

use crate::{debug, info};

use super::{
    node_communication::{NodeCommunication, PeerComunication},
    NodeID, ProtocolEvent,
};

pub struct NodeListener<
    T: Message<Result = anyhow::Result<ProtocolEvent<T>>>
        + Debug
        + Send
        + Serialize
        + DeserializeOwned
        + 'static,
    A: Actor + actix::Handler<T, Result = ResponseActFuture<A, anyhow::Result<ProtocolEvent<T>>>>,
> {
    me: NodeID,
    port: u16,
    ip: Ipv4Addr,
    initial_list: Vec<u16>,
    operator: Addr<A>,

    cancel: Option<CancellationToken>,
    node_comm: Option<Arc<NodeCommunication<T, A>>>,
    task_handle: Option<JoinHandle<anyhow::Result<()>>>,
}

impl<
        T: Message<Result = anyhow::Result<ProtocolEvent<T>>>
            + Debug
            + Send
            + Serialize
            + DeserializeOwned
            + 'static,
        A: Actor + actix::Handler<T, Result = ResponseActFuture<A, anyhow::Result<ProtocolEvent<T>>>>,
    > NodeListener<T, A>
where
    <A as Actor>::Context: ToEnvelope<A, T>,
    <T as actix::Message>::Result: std::marker::Send,
{
    pub fn start(
        port: u16,
        ip: Ipv4Addr,
        me: NodeID,
        initial_list: Vec<u16>,
        operator: Addr<A>,
    ) -> Self {
        let cancel = CancellationToken::new();
        let child_token = cancel.child_token();
        let grand_child_token = child_token.child_token();
        let node_comm = NodeCommunication::start(grand_child_token, me, operator.clone());
        let handle = tokio::spawn(Self::run(
            port,
            ip,
            child_token,
            initial_list.clone(),
            node_comm.clone(),
        ));

        Self {
            port,
            ip,
            initial_list,
            operator,
            cancel: Some(cancel),
            task_handle: Some(handle),
            node_comm: Some(node_comm),
            me,
        }
    }

    pub async fn restart(self) -> Self {
        if let Some(handle) = self.task_handle {
            let _ = handle.await;
        }
        Self::start(
            self.port,
            self.ip,
            self.me,
            self.initial_list,
            self.operator,
        )
    }

    async fn run(
        port: u16,
        ip: Ipv4Addr,
        cancel: CancellationToken,
        initial_list: Vec<u16>,
        node_comm: Arc<NodeCommunication<T, A>>,
    ) -> anyhow::Result<()> {
        let mut node_comms = Self::connect_initial(
            initial_list.into_iter().map(|port| (ip, port)).collect(),
            node_comm.clone(),
        )
        .await;

        let listener = TcpListener::bind(format!("{}:{}", &ip, port)).await?;

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

        info!("Finished listening for new nodes. Sending the cancel signal to all the childs");
        cancel.cancel();
        let _ = futures::future::join_all(node_comms.into_iter().map(|nc| nc.shutdown()));

        Ok(())
    }

    async fn connect_initial<D: Debug + ToSocketAddrs>(
        initial_list: Vec<D>,
        node_comm: Arc<NodeCommunication<T, A>>,
    ) -> Vec<PeerComunication> {
        let tasks = initial_list
            .into_iter()
            .map(|addr| Self::connect_initial_single(addr, node_comm.clone()));

        futures::future::join_all(tasks)
            .await
            .into_iter()
            .filter_map(|conn| conn.ok())
            .collect()
    }

    async fn connect_initial_single<D: Debug + ToSocketAddrs>(
        to: D,
        node_comm: Arc<NodeCommunication<T, A>>,
    ) -> anyhow::Result<PeerComunication> {
        let stream = TcpStream::connect(&to).await?;
        info!(format!("Initial connect to node in {:?}", &to));
        return Ok(node_comm.new_node(stream));
    }

    pub async fn shutdown(self) -> Self {
        info!("Shutting down the listening for new nodes");
        if let Some(cancel) = self.cancel {
            cancel.cancel();
        }
        if let Some(task) = self.task_handle {
            let _ = task.await;
        }
        Self {
            me: self.me,
            port: self.port,
            ip: self.ip,
            initial_list: self.initial_list,
            operator: self.operator,
            cancel: None,
            task_handle: None,
            node_comm: None,
        }
    }

    pub fn is_running(&self) -> bool {
        match &self.task_handle {
            Some(task) => task.is_finished(),
            None => false,
        }
    }

    async fn commmunicate(&self, node: NodeID, msg: T) -> anyhow::Result<T> {
        debug!(format!(
            "Sending a message to {} and waiting for response. Message: {:?}",
            node, msg
        ));
        match &self.node_comm {
            Some(comm) => comm.commmunicate(node, msg).await,
            None => bail!("Message listener is not running"),
        }
    }

    async fn send_message(&self, node: NodeID, msg: T) -> anyhow::Result<()> {
        debug!(format!("Sending a message to {}. Message: {:?}", node, msg));
        match &self.node_comm {
            Some(comm) => comm.send_message(node, msg).await,
            None => bail!("Message listener is not running"),
        }
    }
}
