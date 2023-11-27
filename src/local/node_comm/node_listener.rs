use actix::{dev::ToEnvelope, Actor, Addr, Message, ResponseActFuture};
use anyhow::bail;
use core::fmt::Debug;
use serde::{de::DeserializeOwned, Serialize};
use std::{net::Ipv4Addr, sync::Arc};
use tokio::{
    net::{TcpListener, TcpStream, ToSocketAddrs},
    select,
    sync::Mutex,
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;

use crate::{debug, info};

use super::{
    node_communication::{NodeCommunication, PeerComunication},
    NodeID, ProtocolEvent,
};

#[derive(Clone)]
pub struct NodeListener<
    T: Message<Result = anyhow::Result<ProtocolEvent<T>>>
        + Debug
        + Send
        + Serialize
        + DeserializeOwned
        + 'static,
    A: Actor + actix::Handler<T, Result = ResponseActFuture<A, anyhow::Result<ProtocolEvent<T>>>>,
> {
    pub me: NodeID,
    port: u16,
    ip: Ipv4Addr,
    initial_list: Vec<u16>,
    operator: Addr<A>,

    cancel: Option<CancellationToken>,
    node_comm: Option<Arc<NodeCommunication<T, A>>>,
    task_handle: Option<Arc<Mutex<JoinHandle<anyhow::Result<()>>>>>,
}

impl<
        T: Message<Result = anyhow::Result<ProtocolEvent<T>>>
            + Debug
            + Send
            + Sync
            + Serialize
            + DeserializeOwned
            + 'static,
        A: Actor + actix::Handler<T, Result = ResponseActFuture<A, anyhow::Result<ProtocolEvent<T>>>>,
    > NodeListener<T, A>
where
    <A as Actor>::Context: ToEnvelope<A, T>,
    <T as actix::Message>::Result: std::marker::Send,
{
    pub fn new(
        port: u16,
        ip: Ipv4Addr,
        me: NodeID,
        initial_list: Vec<u16>,
        operator: Addr<A>,
    ) -> Self {
        Self {
            me,
            port,
            ip,
            initial_list,
            operator,

            cancel: None,
            task_handle: None,
            node_comm: None,
        }
    }

    pub fn start(&mut self) {
        let cancel = CancellationToken::new();
        let child_token = cancel.child_token();
        let grand_child_token = child_token.child_token();
        let node_comm = NodeCommunication::start(grand_child_token, self.me, self.operator.clone());
        let handle = tokio::spawn(Self::run(
            self.port,
            self.ip,
            child_token,
            self.initial_list.clone(),
            node_comm.clone(),
        ));

        self.cancel = Some(cancel);
        self.task_handle = Some(Arc::new(Mutex::new(handle)));
        self.node_comm = Some(node_comm);
    }

    pub async fn restart(&mut self) {
        if let Some(handle) = &self.task_handle {
            let _ = handle.as_ref().lock().await;
        }

        self.cancel = None;
        self.task_handle = None;
        self.node_comm = None;
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
        let _ = futures::future::join_all(node_comms.into_iter().map(|nc| nc.shutdown())).await;

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
        Ok(node_comm.new_node(stream))
    }

    pub async fn shutdown(&mut self) {
        info!("Shutting down the listening for new nodes");
        if let Some(cancel) = &self.cancel {
            cancel.cancel();
        }
        if let Some(task) = &self.task_handle {
            let _ = task.as_ref().lock().await;
        }

        self.cancel = None;
        self.task_handle = None;
        self.node_comm = None;
    }

    pub async fn is_running(&self) -> bool {
        match &self.task_handle {
            Some(task) => {
                let task = task.as_ref().lock().await;
                task.is_finished()
            }
            None => false,
        }
    }

    pub async fn commmunicate(&self, node: NodeID, msg: T) -> anyhow::Result<T> {
        debug!(format!(
            "Sending a message to {} and waiting for response. Message: {:?}",
            node, msg
        ));
        match &self.node_comm {
            Some(comm) => comm.commmunicate(node, msg).await,
            None => bail!("Message listener is not running"),
        }
    }

    pub async fn send_message(&self, node: NodeID, msg: T) -> anyhow::Result<()> {
        debug!(format!("Sending a message to {}. Message: {:?}", node, msg));
        match &self.node_comm {
            Some(comm) => comm.send_message(node, msg).await,
            None => bail!("Message listener is not running"),
        }
    }

    pub async fn broadcast(&self, msg: T) -> anyhow::Result<Vec<anyhow::Result<()>>> {
        match &self.node_comm {
            Some(comm) => comm.broadcast(msg).await,
            None => bail!("Message listener is not running"),
        }
    }
}
