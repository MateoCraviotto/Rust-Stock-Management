use std::{str::FromStr, sync::Arc};

use actix::{Actor, ActorFutureExt, Context, Handler, ResponseActFuture, WrapFuture};
use purchases::purchase_state::PurchaseState;
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
    ecommerce::purchases::{self, store::Store},
    info,
};

use super::ListenerState;

pub struct Communication {
    cancel_token: CancellationToken,
}

impl Communication {
    pub fn new(
        stream: TcpStream,
        store: Arc<Mutex<Store>>,
    ) -> (Self, JoinHandle<anyhow::Result<()>>) {
        let mine = CancellationToken::new();
        let pair = mine.clone();
        let arc_stream = Arc::new(Mutex::new(stream));

        let handle = tokio::spawn(serve(arc_stream, pair, store));

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
                Box::pin(async {}.into_actor(self).map(move |_result, _me, _ctx| {
                    return Ok(());
                }))
            }
            _ => Box::pin(async {}.into_actor(self).map(move |_result, _me, _ctx| {
                return Ok(());
            })),
        }
    }
}

async fn serve(
    stream: Arc<Mutex<TcpStream>>,
    token: CancellationToken,
    store: Arc<Mutex<Store>>,
) -> anyhow::Result<()> {
    'serving: loop {
        debug!(format!(
            "Waiting for data or connection or cancellation. {:?}",
            token
        ));
        let store_clone = store.clone();
        select! {
            r = read_socket(stream.clone()) => {
                let data = match r {
                    Ok(d) => d,
                    Err(e) => {
                        info!(format!("Error while reading from socket: {}", e));
                        break 'serving;
                    },
                };
                debug!("Read some data");
                println!("RECEIVED: {}", data);
                match Order::from_str(&data){
                    Ok(o) => {
                        let order_state = store_clone.as_ref().lock().await.manage_order(o);
                        match order_state {
                            PurchaseState::Reserve => {
                                println!("Stock reserved");
                                // Reserve stock
                                println!("Sending reservation ack");
                                let reservation = format!("{}{}",PurchaseState::Reserve.to_string(),"\n");
                                write_socket(stream.clone(), &reservation).await?;

                            },
                            _ => {
                                info!(format!("Store has not enough stock of product {}. Cancelling purchase.", o.get_product()));
                                let cancellation = PurchaseState::Cancel.to_string();
                                write_socket(stream.clone(), &cancellation).await?;
                            },

                        }
                    },
                    Err(_) => {} // It is not an order, do nothing
                };
                match PurchaseState::from_str(&data) {
                    Ok(purchase_state) => {
                        match purchase_state {
                            PurchaseState::Reserve => {
                                println!("Stock reserved");
                                // Reserve stock
                                println!("Sending reservation ack");
                                write_socket(stream.clone(), &PurchaseState::Reserve.to_string()).await?;

                            },
                            PurchaseState::Confirm => {
                                println!("Purchase confirmed from ecommerce. Sending final confirmation.");
                                write_socket(stream.clone(), &PurchaseState::Confirm.to_string()).await?;
                            }
                            _ => {
                                println!("Purchase was not confirmed. Cancelling purchase.");
                            },

                        }
                    },
                    Err(_) => continue, // Do nothing
                };
                //data = String::new();
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
