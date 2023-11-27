use crate::local::protocol::store_glue::ProtocolStoreMessage;
use actix::dev::ToEnvelope;
use actix::{Actor, Addr, Message, ResponseActFuture};
use anyhow::{anyhow, bail};
use rand::Rng;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::fmt::Debug;
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{tcp::OwnedWriteHalf, TcpStream},
    select,
    sync::{
        mpsc::{self, Receiver, Sender},
        Mutex, Notify,
    },
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;

use crate::local::NodeID;
use crate::{debug, error};

use super::ProtocolEvent;

type CorrelationID = u64;

#[derive(Serialize, Deserialize, Debug)]
enum MessageType {
    Request,
    Response,
    Error,
}

#[derive(Serialize, Deserialize, Debug, Message)]
#[rtype(result = "anyhow::Result<ProtocolEvent<ProtocolStoreMessage>>")]
pub struct InterNodeMessage<T> {
    from: NodeID,
    correlation_id: Option<CorrelationID>,
    message_type: MessageType,
    body: T,
}
pub enum PeerMessage {
    PeerUp(NodeID, OwnedWriteHalf),
    PeerResponse(NodeID, Vec<u8>),
    PeerDown(NodeID),
}

pub struct NodeCommunication<
    T: Message + Debug + Send + Serialize + DeserializeOwned + 'static,
    A: Actor + actix::Handler<T>,
> {
    me: NodeID,
    cancel: CancellationToken,
    peer_task_rx: Mutex<Receiver<PeerMessage>>,
    peer_task_tx: Sender<PeerMessage>,
    peers_writer: Mutex<HashMap<NodeID, OwnedWriteHalf>>,
    responses: Arc<Mutex<HashMap<CorrelationID, (Notify, Option<T>)>>>,
    actor: Addr<A>,
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
    > NodeCommunication<T, A>
where
    <A as Actor>::Context: ToEnvelope<A, T>,
    <T as actix::Message>::Result: std::marker::Send,
{
    pub fn start(cancel: CancellationToken, me: NodeID, actor: Addr<A>) -> Arc<Self> {
        let (peer_tx, peer_rx) = mpsc::channel(256);
        let this = Arc::new(Self {
            me,
            cancel,
            peer_task_rx: Mutex::new(peer_rx),
            peer_task_tx: peer_tx,
            peers_writer: Mutex::new(HashMap::new()),
            responses: Arc::new(Mutex::new(HashMap::new())),
            actor,
        });

        let _ = tokio::spawn(Self::run(this.clone()));

        this
    }

    async fn run(this: Arc<Self>) -> anyhow::Result<()> {
        let mut rx = this.peer_task_rx.lock().await;
        loop {
            select! {
                r = rx.recv() => {
                    match r{
                        Some(m) => {
                            match m{
                                PeerMessage::PeerUp(id, writer) => {
                                    this.peers_writer.lock().await.insert(id, writer);
                                },
                                PeerMessage::PeerDown(id) => {
                                    this.peers_writer.lock().await.remove(&id);
                                },
                                PeerMessage::PeerResponse(id, serialized) => {
                                    let _ = this.send_bytes(id, &serialized).await;
                                }
                            }
                        },
                        None => break,
                    }
                }

                _ = this.cancel.cancelled() => {
                    break;
                }
            }
        }

        Ok(())
    }

    pub fn new_node(&self, stream: TcpStream) -> PeerComunication {
        let tx = self.peer_task_tx.clone();

        PeerComunication::new::<T, A>(
            self.me,
            stream,
            self.cancel.clone(),
            tx,
            self.actor.clone(),
            self.responses.clone(),
        )
    }

    pub async fn commmunicate(&self, node: NodeID, msg: T) -> anyhow::Result<T> {
        let correlation_id = self.generate_correlation_id();
        let with_header = InterNodeMessage {
            from: self.me,
            correlation_id: Some(correlation_id),
            message_type: MessageType::Request,
            body: msg,
        };
        self.send(node, with_header).await?;

        let response = self.get_response(correlation_id).await?;

        Ok(response)
    }

    pub async fn send_message(&self, node: NodeID, msg: T) -> anyhow::Result<()> {
        let correlation_id = None;
        let with_header = InterNodeMessage {
            from: self.me,
            correlation_id,
            message_type: MessageType::Request,
            body: msg,
        };
        self.send(node, with_header).await
    }

    pub async fn broadcast(&self, msg: T) -> anyhow::Result<Vec<anyhow::Result<()>>> {
        let correlation_id = None;
        let with_header = InterNodeMessage {
            from: self.me,
            correlation_id,
            message_type: MessageType::Request,
            body: msg,
        };
        let bytes = &serde_json::to_vec(&with_header)?;
        let keys: Vec<NodeID> = {
            let lock = self.responses.lock().await;
            let r = lock.keys().copied().collect();
            drop(lock);
            r
        };

        Ok(
            futures::future::join_all(keys.into_iter().map(|key| self.send_bytes(key, &bytes)))
                .await,
        )
    }

    async fn send(&self, node: NodeID, msg: InterNodeMessage<T>) -> anyhow::Result<()> {
        self.send_bytes(node, &serde_json::to_vec(&msg)?).await
    }

    async fn send_bytes(&self, node: NodeID, msg: &[u8]) -> anyhow::Result<()> {
        match self.peers_writer.lock().await.get_mut(&node) {
            Some(writer) => {
                writer.write_all(msg).await?;
                Ok(())
            }
            None => bail!("NodeID {} is not registered", node),
        }
    }

    async fn get_response(&self, correlation_id: CorrelationID) -> anyhow::Result<T> {
        let mut map = self.responses.lock().await;
        if map.get(&correlation_id).is_none() {
            map.insert(correlation_id, (Notify::new(), None));
        }

        let res = {
            let mut timeout = false;
            while map.get(&correlation_id).unwrap().1.is_none() {
                select! {
                    _ = map.get(&correlation_id).unwrap().0.notified() => {
                        break;
                    }

                    _ = tokio::time::sleep(Duration::from_secs(1)) => {
                        // If this happens, it means that the connection is probably down
                        // but this shouldn't be decided here, let the stream be the one to decide.
                        timeout = true;
                        break;
                    }
                }
            }

            timeout
        };

        if res {
            bail!("Timeout while waiting for a response")
        }

        let response = map
            .remove(&correlation_id)
            .ok_or(anyhow!("Error while getting a supossedly existant value"))?
            .1
            .ok_or(anyhow!("Error while getting a supossedly existant value"))?;

        Ok(response)
    }

    fn generate_correlation_id(&self) -> CorrelationID {
        rand::thread_rng().gen::<CorrelationID>() >> 32 & ((self.me >> 32) << 32)
    }
}

pub struct PeerComunication {
    task_handle: JoinHandle<anyhow::Result<()>>,
}

impl PeerComunication {
    pub fn new<
        T: Message<Result = anyhow::Result<ProtocolEvent<T>>>
            + Debug
            + Send
            + Sync
            + Serialize
            + DeserializeOwned
            + 'static,
        A: Actor + actix::Handler<T, Result = ResponseActFuture<A, anyhow::Result<ProtocolEvent<T>>>>,
    >(
        me: NodeID,
        stream: TcpStream,
        cancel: CancellationToken,
        tx: Sender<PeerMessage>,
        actor: Addr<A>,
        response_bus: Arc<Mutex<HashMap<CorrelationID, (Notify, Option<T>)>>>,
    ) -> Self
    where
        <A as Actor>::Context: ToEnvelope<A, T>,
        <T as actix::Message>::Result: std::marker::Send,
    {
        Self {
            task_handle: tokio::spawn(Self::run(me, stream, cancel, tx, actor, response_bus)),
        }
    }

    async fn run<
        T: Message<Result = anyhow::Result<ProtocolEvent<T>>>
            + Debug
            + Send
            + Sync
            + Serialize
            + DeserializeOwned
            + 'static,
        A: Actor + actix::Handler<T, Result = ResponseActFuture<A, anyhow::Result<ProtocolEvent<T>>>>,
    >(
        me: NodeID,
        stream: TcpStream,
        cancel: CancellationToken,
        tx: Sender<PeerMessage>,
        actor: Addr<A>,
        response_bus: Arc<Mutex<HashMap<CorrelationID, (Notify, Option<T>)>>>,
    ) -> anyhow::Result<()>
    where
        <A as Actor>::Context: ToEnvelope<A, T>,
        <T as actix::Message>::Result: std::marker::Send,
    {
        let (mut read, write) = stream.into_split();
        let mut writer_sent = Some(write);
        let mut id = None;

        loop {
            let mut buffer = vec![0; 1 << 16];
            select! {
                r = read.read(&mut buffer) => {
                    if r.is_err() {
                        break;
                    }
                    let n = r.unwrap();
                    if n == 0 {
                        break;
                    }
                    let parsed: Result<InterNodeMessage<T>, serde_json::Error> = serde_json::from_slice(&buffer[0..n]);

                    match parsed{
                        Ok(protocol_message) => {
                            let from_id = protocol_message.from;
                            let correlation = protocol_message.correlation_id;
                            let t = &protocol_message.message_type;

                            match (t, correlation) {
                                (MessageType::Request, corr) => {
                                        match actor.send(protocol_message.body).await{
                                            Ok(Ok(a)) => {
                                                match a{
                                                    ProtocolEvent::MaybeNew => {
                                                        if let Some(writer) = writer_sent{
                                                            let _ = tx.send(PeerMessage::PeerUp(from_id, writer)).await;
                                                            id = Some(from_id);
                                                            writer_sent = None;
                                                        }
                                                    },
                                                    ProtocolEvent::Response(response) => {
                                                        Self::send_back(&tx, from_id, InterNodeMessage{
                                                            from: me,
                                                            correlation_id: corr,
                                                            message_type: MessageType::Response,
                                                            body: response
                                                        }).await;
                                                    }
                                                    ProtocolEvent::Teardown => {
                                                        break;
                                                    }
                                                    _ => {},
                                                }
                                            },
                                            Ok(Err(e)) => Self::send_back(&tx, from_id, InterNodeMessage {
                                                from: me,
                                                correlation_id: None,
                                                message_type: MessageType::Error,
                                                body: format!("{:?}", e)
                                            }).await,
                                            Err(_) => Self::send_back(&tx, from_id, InterNodeMessage {
                                                from: me,
                                                correlation_id: None,
                                                message_type: MessageType::Error,
                                                body: "Server error"
                                            }).await,
                                        };
                                },
                                (MessageType::Response | MessageType::Error, Some(corr_id)) => {
                                    let mut responses = response_bus.lock().await;
                                    let notif = Notify::new();
                                    notif.notify_one();
                                    if let Some((old_notif, _)) = responses.insert(corr_id, (notif, Some(protocol_message.body))){
                                        old_notif.notify_one()
                                    }
                                },
                                (MessageType::Response | MessageType::Error, None) => {
                                    error!(format!("There was a response with no correlation ID. Ignoring it {:?}", &protocol_message))
                                }
                            }
                        }
                        Err(e) => {
                            debug!(format!("Error al parsear un mensaje: {}", e));
                        }
                    }
                }

                _ = cancel.cancelled() => {
                    break;
                }
            }
        }

        if let Some(from_id) = id {
            let _ = tx.send(PeerMessage::PeerDown(from_id)).await;
        }

        Ok(())
    }

    pub async fn shutdown(self) -> anyhow::Result<()> {
        self.task_handle.await?
    }

    async fn send_back<S: Serialize>(
        tx: &Sender<PeerMessage>,
        to: NodeID,
        msg: InterNodeMessage<S>,
    ) {
        if let Ok(s) = serde_json::to_vec(&msg) {
            let _ = tx.send(PeerMessage::PeerResponse(to, s)).await;
        }
    }
}
