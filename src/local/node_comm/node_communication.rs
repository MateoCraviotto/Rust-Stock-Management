use anyhow::{anyhow, bail};
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

use crate::debug;
use crate::local::node_comm::ProtocolEvent;
use crate::local::NodeID;

use super::Operator;

type CorrelationID = u64;

#[derive(Serialize, Deserialize, Debug)]
pub struct InterNodeMessage<T> {
    from: NodeID,
    correlation_id: Option<CorrelationID>,
    body: T,
}
pub enum PeerMessage {
    PeerUp(NodeID, OwnedWriteHalf),
    PeerDown(NodeID),
}

pub struct NodeCommunication<
    T: Debug + Send + Serialize + DeserializeOwned + 'static,
    O: Operator<T> + 'static,
> {
    me: NodeID,
    cancel: CancellationToken,
    peer_task_rx: Mutex<Receiver<PeerMessage>>,
    peer_task_tx: Sender<PeerMessage>,
    peers_writer: Mutex<HashMap<NodeID, OwnedWriteHalf>>,
    responses: Arc<Mutex<HashMap<CorrelationID, (Notify, Option<T>)>>>,
    operator: Arc<O>,
}

impl<T: Debug + Send + Serialize + DeserializeOwned + 'static, O: Operator<T>>
    NodeCommunication<T, O>
{
    pub fn start(cancel: CancellationToken, me: NodeID, operator: Arc<O>) -> Arc<Self> {
        let (peer_tx, peer_rx) = mpsc::channel(256);
        let this = Arc::new(Self {
            me,
            cancel,
            peer_task_rx: Mutex::new(peer_rx),
            peer_task_tx: peer_tx,
            peers_writer: Mutex::new(HashMap::new()),
            responses: Arc::new(Mutex::new(HashMap::new())),
            operator: operator,
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

        PeerComunication::new::<T, O>(
            stream,
            self.cancel.clone(),
            tx,
            self.operator.clone(),
            self.responses.clone(),
        )
    }

    pub async fn commmunicate(&self, node: NodeID, msg: T) -> anyhow::Result<T> {
        let correlation_id = Some(Self::generate_correlation_id());
        let with_header = InterNodeMessage {
            from: self.me,
            correlation_id,
            body: msg,
        };
        self.send(node, with_header).await?;

        let response = self
            .get_response(correlation_id.expect("Solar radiaton or what"))
            .await?;

        Ok(response)
    }

    pub async fn send_message(&self, node: NodeID, msg: T) -> anyhow::Result<()> {
        let correlation_id = None;
        let with_header = InterNodeMessage {
            from: self.me,
            correlation_id,
            body: msg,
        };
        self.send(node, with_header).await
    }

    async fn send(&self, node: NodeID, msg: InterNodeMessage<T>) -> anyhow::Result<()> {
        match self.peers_writer.lock().await.get_mut(&node) {
            Some(writer) => {
                let to_write = serde_json::to_string(&msg)?;
                writer.write_all(to_write.as_bytes()).await?;
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

    fn generate_correlation_id() -> CorrelationID {
        return 1;
    }
}

pub struct PeerComunication {
    task_handle: JoinHandle<anyhow::Result<()>>,
}

impl PeerComunication {
    pub fn new<T: Send + Serialize + DeserializeOwned + 'static, O: Operator<T> + 'static>(
        stream: TcpStream,
        cancel: CancellationToken,
        tx: Sender<PeerMessage>,
        operator: Arc<O>,
        response_bus: Arc<Mutex<HashMap<CorrelationID, (Notify, Option<T>)>>>,
    ) -> Self {
        Self {
            task_handle: tokio::spawn(Self::run(stream, cancel, tx, operator, response_bus)),
        }
    }

    async fn run<T: Send + Serialize + DeserializeOwned + 'static, O: Operator<T>>(
        stream: TcpStream,
        cancel: CancellationToken,
        tx: Sender<PeerMessage>,
        operator: Arc<O>,
        response_bus: Arc<Mutex<HashMap<CorrelationID, (Notify, Option<T>)>>>,
    ) -> anyhow::Result<()> {
        let (mut read, write) = stream.into_split();
        let mut writer_sent = Some(write);
        let mut id = None;

        loop {
            let mut buffer = vec![0; 1 << 16];
            select! {
                r = read.read(&mut buffer) => {
                    if let Err(_) = r{
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

                            match correlation{
                                Some(correlation_id) => {
                                    // Es posible notifcar apenas se crea porque: Se tiene el lock, las notificaciones se guardan
                                    let mut responses = response_bus.lock().await;
                                    let notif = Notify::new();
                                    notif.notify_one();
                                    if let Some((old_notif, _)) = responses.insert(correlation_id, (notif, Some(protocol_message.body))){
                                        old_notif.notify_one()
                                    }
                                },
                                None => {
                                    match operator.handle(from_id, protocol_message.body){
                                        Ok(ProtocolEvent::MaybeNew) => {
                                            if let Some(writer) = writer_sent{
                                                let _ = tx.send(PeerMessage::PeerUp(from_id, writer));
                                                id = Some(from_id);
                                                writer_sent = None;
                                            }
                                        },
                                        Ok(ProtocolEvent::Response(_)) => {
                                            //TODO: mandaselo devuelta, de alguna forma
                                        }
                                        Ok(ProtocolEvent::Teardown) => {
                                            break;
                                        }
                                        Err(_) => todo!(),
                                    }
                                },
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
            let _ = tx.send(PeerMessage::PeerDown(from_id));
        }

        todo!()
    }

    pub async fn shutdown(self) -> anyhow::Result<()> {
        self.task_handle.await?
    }
}
