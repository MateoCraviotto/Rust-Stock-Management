use crate::local::protocol::store_glue::ProtocolStoreMessage;
use actix::dev::ToEnvelope;
use actix::{Actor, Addr, Message, ResponseActFuture};
use anyhow::bail;
use rand::Rng;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::fmt::Debug;
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::sync::oneshot;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{tcp::OwnedWriteHalf, TcpStream},
    select,
    sync::{
        mpsc::{self, Receiver, Sender},
        Mutex,
    },
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;

use crate::error;
use crate::local::NodeID;

use super::{ActorLifetime, ProtocolEvent};

type CorrelationID = u64;

#[derive(Serialize, Deserialize, Debug)]
enum MessageType {
    Hello,
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
    body: Option<T>,
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
    responses: Arc<Mutex<HashMap<CorrelationID, oneshot::Sender<T>>>>,
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
        A: Actor
            + actix::Handler<T, Result = ResponseActFuture<A, anyhow::Result<ProtocolEvent<T>>>>
            + actix::Handler<ActorLifetime, Result = ()>,
    > NodeCommunication<T, A>
where
<A as Actor>::Context: ToEnvelope<A, T>,
<A as Actor>::Context: ToEnvelope<A, ActorLifetime>,
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
            body: Some(msg),
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
            body: Some(msg),
        };
        self.send(node, with_header).await
    }

    pub async fn broadcast(&self, msg: T) -> anyhow::Result<Vec<anyhow::Result<()>>> {
        let correlation_id = None;
        let with_header = InterNodeMessage {
            from: self.me,
            correlation_id,
            message_type: MessageType::Request,
            body: Some(msg),
        };
        let bytes = &serde_json::to_vec(&with_header)?;
        let keys: Vec<NodeID> = {
            let lock = self.peers_writer.lock().await;
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
        let message = &serde_json::to_vec(&msg)?;
        self.send_bytes(node, message).await
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
        let (tx, rx) = oneshot::channel();
        self.responses.lock().await.insert(correlation_id, tx);

        select! {
            res = rx => {
                return Ok(res?);
            }

            _ = tokio::time::sleep(Duration::from_secs(1)) => {
                bail!("Timeout while waiting for a response")
            }
        }
    }

    fn generate_correlation_id(&self) -> CorrelationID {
        rand::thread_rng().gen()
    }
}

#[derive(Debug)]
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
        A: Actor
            + actix::Handler<T, Result = ResponseActFuture<A, anyhow::Result<ProtocolEvent<T>>>>
            + actix::Handler<ActorLifetime, Result = ()>,
    >(
        me: NodeID,
        stream: TcpStream,
        cancel: CancellationToken,
        tx: Sender<PeerMessage>,
        actor: Addr<A>,
        response_bus: Arc<Mutex<HashMap<CorrelationID, oneshot::Sender<T>>>>,
    ) -> Self
    where
    <A as Actor>::Context: ToEnvelope<A, T>,
    <A as Actor>::Context: ToEnvelope<A, ActorLifetime>,
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
        A: Actor
            + actix::Handler<T, Result = ResponseActFuture<A, anyhow::Result<ProtocolEvent<T>>>>
            + actix::Handler<ActorLifetime, Result = ()>,
    >(
        me: NodeID,
        stream: TcpStream,
        cancel: CancellationToken,
        tx: Sender<PeerMessage>,
        actor: Addr<A>,
        response_bus: Arc<Mutex<HashMap<CorrelationID, oneshot::Sender<T>>>>,
    ) -> anyhow::Result<()>
    where
        <A as Actor>::Context: ToEnvelope<A, T>,
        <A as Actor>::Context: ToEnvelope<A, ActorLifetime>,
        <T as actix::Message>::Result: std::marker::Send,
    {
        let (mut read, write) = stream.into_split();
        let mut writer_sent = Some(Self::hello::<T>(write, me).await);
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
                                    if let Some(body) = protocol_message.body {
                                        println!("Received REQUEST FROM OTHER NODE: {:?}", body);
                                        let response = actor.send(body).await?;
                                        println!("response: {:?}", response);
                                        match response {
                                            Ok(a) => {
                                                match a {
                                                    ProtocolEvent::MaybeNew => {
                                                        if let Some(writer) = writer_sent{
                                                            let _ = tx.send(PeerMessage::PeerUp(from_id, writer)).await;
                                                            id = Some(from_id);
                                                            writer_sent = None;
                                                        }
                                                    },
                                                    ProtocolEvent::Response(response) => {
                                                        println!("RESPONDING TO NODE WITH {:?}", response);
                                                        Self::send_back(&tx, from_id, InterNodeMessage{
                                                            from: me,
                                                            correlation_id: corr,
                                                            message_type: MessageType::Response,
                                                            body: Some(response)
                                                        }).await;
                                                    }
                                                    ProtocolEvent::Teardown => {
                                                        break;
                                                    }
                                                    _ => {},
                                                }
                                            },
                                            Err(e) => Self::send_back(&tx, from_id, InterNodeMessage {
                                                from: me,
                                                correlation_id: None,
                                                message_type: MessageType::Error,
                                                body: Some(format!("{:?}", e))
                                            }).await,
                                            /*Err(_) => Self::send_back(&tx, from_id, InterNodeMessage {
                                                from: me,
                                                correlation_id: None,
                                                message_type: MessageType::Error,
                                                body: Some("Server error")
                                            }).await,
                                            */
                                        };
                                    }
                                },
                                (MessageType::Response | MessageType::Error, Some(corr_id)) => {
                                    let _ = response_bus.lock().await.remove(&corr_id).is_some_and(|s| {
                                        if let Some(response) = protocol_message.body {
                                            s.send(response);
                                        }
                                        true
                                    });
                                },
                                (MessageType::Response | MessageType::Error, None) => {
                                    error!(format!("There was a response with no correlation ID. Ignoring it {:?}", &protocol_message))
                                },
                                (MessageType::Hello, _) => {
                                    if let Some(writer) = writer_sent{
                                        let _ = tx.send(PeerMessage::PeerUp(from_id, writer)).await;
                                        id = Some(from_id);
                                        writer_sent = None;
                                    }
                                }
                            }
                        }
                        Err(_) => {
                            //debug!(format!("Error al parsear un mensaje: {}", e));
                        }
                    }
                }

                _ = cancel.cancelled() => {
                    break;
                }
            }
        }

        if let Some(from_id) = id {
            actor.do_send(ActorLifetime::Shutdown(from_id));
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

    async fn hello<T: Debug + Serialize + DeserializeOwned + 'static>(
        mut writer: OwnedWriteHalf,
        me: NodeID,
    ) -> OwnedWriteHalf {
        let hello_msg: InterNodeMessage<T> = InterNodeMessage {
            from: me,
            correlation_id: None,
            message_type: MessageType::Hello,
            body: None,
        };
        if let Ok(msg) = serde_json::to_vec(&hello_msg) {
            let _ = writer.write_all(&msg).await;
        }
        writer
    }
}
