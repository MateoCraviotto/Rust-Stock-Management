use std::{collections::HashMap, str::FromStr, sync::Arc};

use actix::{Actor, ActorFutureExt, Addr, Context, Handler, ResponseActFuture, WrapFuture};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    select,
    sync::Mutex,
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;

use crate::{
    common::order::Order,
    debug,
    ecommerce::purchases::{
        messages::{MessageType, StoreMessage},
        purchase_state::PurchaseState,
        store::{Stock, StoreActor, Transaction, TransactionState},
    },
    info,
    local::{
        node_comm::node_listener::NodeListener,
        protocol::{
            messages::{
                NodeModification, ProtocolMessage, ProtocolMessageType, Request, RequestAction,
            },
            store_glue::{AbsoluteStateUpdate, StoreGlue},
        },
    },
};

use super::ListenerState;

pub struct Communication {
    cancel_token: CancellationToken,
}

impl Communication {
    pub fn new(
        stream: TcpStream,
        store: Addr<StoreActor>,
        arc_internal_listener: Arc<
            Mutex<NodeListener<ProtocolMessage<HashMap<u64, u64>, AbsoluteStateUpdate>, StoreGlue>>,
        >,
    ) -> (Self, JoinHandle<anyhow::Result<()>>) {
        let mine = CancellationToken::new();
        let pair = mine.clone();
        let arc_stream = Arc::new(Mutex::new(stream));

        let handle = tokio::spawn(serve(arc_stream, pair, store, arc_internal_listener));

        (Self { cancel_token: mine }, handle)
    }
}

impl Actor for Communication {
    type Context = Context<Self>;
}

impl Handler<ListenerState> for Communication {
    type Result = ResponseActFuture<Self, anyhow::Result<()>>;

    fn handle(&mut self, msg: ListenerState, _ctx: &mut Self::Context) -> Self::Result {
        match msg {
            ListenerState::Down | ListenerState::Shutdown => {
                self.cancel_token.cancel();
                debug!(format!(
                    "Sending cancellation token for connection, {:?}",
                    self.cancel_token
                ));
                Box::pin(
                    async {}
                        .into_actor(self)
                        .map(move |_result, _me, _ctx| Ok(())),
                )
            }
            _ => Box::pin(
                async {}
                    .into_actor(self)
                    .map(move |_result, _me, _ctx| Ok(())),
            ),
        }
    }
}

async fn serve(
    stream: Arc<Mutex<TcpStream>>,
    token: CancellationToken,
    store: Addr<StoreActor>,
    arc_internal_listener: Arc<
        Mutex<NodeListener<ProtocolMessage<HashMap<u64, u64>, AbsoluteStateUpdate>, StoreGlue>>,
    >,
) -> anyhow::Result<()> {
    'serving: loop {
        debug!(format!(
            "Waiting for data or connection or cancellation. {:?}",
            token
        ));
        select! {
            r = read_socket(stream.clone()) => {
                let data = match r {
                    Ok(d) => d,
                    Err(_) => {
                        break 'serving;
                    },
                };
                debug!("Read some data");
                println!("RECEIVED: {}", data);
                match Order::from_str(&data) {
                    Ok(order) => {
                        let message = StoreMessage {
                            message_type: MessageType::Request,
                            transactions: None,
                            new_stock: None,
                            orders: Some(vec![order]),
                        };
                        let order_state = store.send(message).await?;
                        match order_state {
                            Some(transaction) => {

                                if transaction.state == TransactionState::Cancelled {
                                    info!("Purchase cancelled");
                                    // Notify ecommerce
                                    write_socket(stream.clone(), &PurchaseState::Cancel(transaction.id).to_string()).await?;
                                    continue
                                }

                                let node_modifications: Vec<NodeModification<_>> = transaction.involved_stock.iter().map(|(store_id, stock)| {
                                    NodeModification {
                                        affected: *store_id,
                                        modifications: vec![stock.clone()],
                                    }
                                }).collect();
                                let mut purchase_cancelled = false;
                                let me = {
                                    let internal_listener = arc_internal_listener.as_ref().lock().await;
                                    let me_clone = internal_listener.me;
                                    drop(internal_listener);
                                    me_clone
                                };

                                let node_request = get_node_request_with(Some(node_modifications), transaction.id,
                                                                         RequestAction::Ask, me);
                                // Send request to each node involved in the purchase
                                for (store_id, _) in transaction.involved_stock.clone() {
                                    let internal_listener = arc_internal_listener.as_ref().lock().await;
                                    if store_id != internal_listener.me {
                                        let response = internal_listener.commmunicate(store_id, node_request.clone()).await?;
                                        info!(format!("Request sent to store {}", store_id));
                                        if let Some(response_info) = response.request_information {
                                            if response_info.request_state == RequestAction::Cancel {
                                                // Purchase cancelled, notify ecommerce
                                                purchase_cancelled = true;
                                            }
                                        }
                                    }
                                }
                                if purchase_cancelled {
                                    info!("Purchase cancelled");
                                    let cancellation_message = StoreMessage {
                                        message_type: MessageType::Cancel(transaction.id),
                                        transactions: None,
                                        new_stock: None,
                                        orders: None,
                                    };
                                    store.send(cancellation_message).await?;
                                    notify_nodes(RequestAction::Cancel, transaction.clone(), arc_internal_listener.clone()).await?;
                                    // Notify ecommerce
                                    write_socket(stream.clone(), &PurchaseState::Cancel(transaction.id).to_string()).await?;
                                } else {
                                    info!("Purchase confirmed");
                                    let confirmation_message = StoreMessage {
                                        message_type: MessageType::Confirm(transaction.id),
                                        transactions: None,
                                        new_stock: None,
                                        orders: None,
                                    };
                                    store.send(confirmation_message).await?;
                                    notify_nodes(RequestAction::Confirm, transaction.clone(), arc_internal_listener.clone()).await?;
                                    // Notify ecommerce
                                    write_socket(stream.clone(), &PurchaseState::Confirm(transaction.id).to_string()).await?;
                                }
                            },
                            None => {
                                info!("Not enough stock to complete purchase. Cancelling purchase.");
                                continue
                            },

                        }

                    },
                    Err(_) => {
                        // do nothing, it's not an order
                    }
                }
                match PurchaseState::from_str(&data) {
                    Ok(purchase_state) => {
                        match purchase_state {
                            PurchaseState::Commit(id) => {
                                println!("Purchase confirmed from ecommerce. Notifying all involved stores.");
                                let message = StoreMessage {
                                    message_type: MessageType::Commit(id),
                                    transactions: None,
                                    new_stock: None,
                                    orders: None,
                                };
                                if let Some(transaction) = store.send(message).await? {
                                    notify_nodes(RequestAction::Commit, transaction.clone(), arc_internal_listener.clone()).await?;
                                    // Notify ecommerce that purchase is completed
                                    write_socket(stream.clone(), &PurchaseState::Commit(id).to_string()).await?;
                                }
                            }
                            PurchaseState::Cancel(id) => {
                                println!("Purchase was cancelled by ecommerce. Notifying all involved stores.");
                                let message = StoreMessage {
                                    message_type: MessageType::Cancel(id),
                                    transactions: None,
                                    new_stock: None,
                                    orders: None,
                                };
                                if let Some(transaction) = store.send(message).await? {
                                    notify_nodes(RequestAction::Cancel, transaction.clone(), arc_internal_listener.clone()).await?;
                                    // Notify ecommerce that purchase was cancelled
                                    write_socket(stream.clone(), &PurchaseState::Cancel(id).to_string()).await?;
                                }
                            },
                            _ => {
                            },
                        }
                    },
                    Err(_) => {
                        continue
                    }, // Do nothing
                };
            }

            _ = token.cancelled() => {
                info!("Closing a connection");
                break 'serving;
            }
        }
    }

    Ok(())
}

pub const BUFSIZE: usize = 512;

/// Function that writes the socket received.
/// # Arguments
/// * `arc_socket` - The socket to write to.
/// * `message` - The message to write.
pub async fn write_socket(arc_socket: Arc<Mutex<TcpStream>>, message: &str) -> anyhow::Result<()> {
    let mut msg = message.to_owned().into_bytes();
    msg.resize(BUFSIZE, 0);
    arc_socket.as_ref().lock().await.write_all(&msg).await?;
    Ok(())
}

/// Function that reads the socket received. It returs
/// the message read in a String.
/// # Arguments
/// * `arc_socket` - The socket to read from.
pub async fn read_socket(arc_socket: Arc<Mutex<TcpStream>>) -> anyhow::Result<String> {
    let mut buff = [0u8; BUFSIZE];
    arc_socket
        .as_ref()
        .lock()
        .await
        .read_exact(&mut buff)
        .await?;
    let msg = buff.into_iter().take_while(|&x| x != 0).collect::<Vec<_>>();
    Ok(String::from_utf8_lossy(&msg).to_string())
}

fn get_node_request_with(
    node_modifications: Option<Vec<NodeModification<HashMap<u64, u64>>>>,
    request_id: u128,
    action: RequestAction,
    me: u64,
) -> ProtocolMessage<Stock, AbsoluteStateUpdate> {
    ProtocolMessage {
        from: me,
        message_type: ProtocolMessageType::Request,
        request_information: Some(Request {
            request_id,
            requester: me,
            request_state: action,
            information: node_modifications,
        }),
        update_information: None,
    }
}

fn get_node_modifications_for(
    involved_stock: HashMap<u64, Stock>,
) -> Vec<NodeModification<HashMap<u64, u64>>> {
    involved_stock
        .iter()
        .map(|(store_id, stock)| NodeModification {
            affected: *store_id,
            modifications: vec![stock.clone()],
        })
        .collect()
}

async fn notify_nodes(
    action: RequestAction,
    transaction: Transaction,
    arc_internal_listener: Arc<
        Mutex<NodeListener<ProtocolMessage<HashMap<u64, u64>, AbsoluteStateUpdate>, StoreGlue>>,
    >,
) -> anyhow::Result<()> {
    let internal_listener = arc_internal_listener.as_ref().lock().await;
    let me = internal_listener.me;
    for (store_id, _) in transaction.involved_stock.clone() {
        if store_id != me {
            let node_modifications: Vec<NodeModification<_>> =
                get_node_modifications_for(transaction.involved_stock.clone());
            let node_request =
                get_node_request_with(Some(node_modifications), transaction.id, action.clone(), me);
            internal_listener
                .send_message(store_id, node_request)
                .await?;
        }
    }

    Ok(())
}
