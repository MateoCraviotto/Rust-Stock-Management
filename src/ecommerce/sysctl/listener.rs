use std::{str::FromStr, sync::Arc, time::Duration};

use actix::Addr;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    select,
    sync::Mutex,
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;

use crate::{
    common::order::read_orders,
    ecommerce::{
        network::{listen::Listener, ListenerState},
        purchases::{
            messages::{MessageType, StoreMessage, StoreState},
            store::{Stock, StoreActor, StoreInformation},
        },
        sysctl::command::Command,
    },
    error, info,
    local::{
        node_comm::node_listener::NodeListener,
        protocol::{
            messages::{ProtocolMessage, ProtocolMessageType},
            store_glue::{AbsoluteStateUpdate, StoreGlue},
        },
        NodeID,
    },
};

pub async fn listen_commands(
    net: Addr<Listener>,
    arc_int_net: Arc<Mutex<NodeListener<ProtocolMessage<Stock, AbsoluteStateUpdate>, StoreGlue>>>,
    store: Addr<StoreActor>,
) -> anyhow::Result<()> {
    println!("Reading commands from STDIN");

    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    let cancel = CancellationToken::new();
    let updater = setup_updater(cancel.clone(), arc_int_net.clone(), store.clone());

    // Save all the times that we tell the network to go up so we wait on all of them.
    // All of them should finish at the same time because of the cancellation tokens
    let mut t = vec![];
    t.push(start_listener(net.clone()));

    'main: loop {
        let r = lines.next_line().await;
        if let Ok(Some(line)) = r {
            let mut int_net = arc_int_net.as_ref().lock().await;
            match Command::from_str(&line) {
                Ok(c) => match c {
                    Command::Shutdown => {
                        info!("Shutdown order was given");
                        net.do_send(ListenerState::Shutdown);
                        int_net.shutdown().await;
                        break 'main;
                    }
                    Command::NetUp => {
                        info!("Network Up order was given");
                        t.push(start_listener(net.clone()));
                        if !int_net.is_running().await {
                            int_net.restart().await;
                        }
                        println!("Network was connected");
                    }
                    Command::NetDown => {
                        info!("Network Down order was given");
                        net.do_send(ListenerState::Down);
                        // Await all current messages to have a 'clean' environment
                        // in next iteration. Will block until all disconnections happened
                        let _ = futures::future::join_all(t).await;
                        t = vec![];

                        if int_net.is_running().await {
                            int_net.shutdown().await;
                        }

                        println!("Network was disconnected");
                    }
                    Command::Sell(o) => {
                        info!(format!("New order was issued: {:?}", o));
                        store.do_send(StoreMessage {
                            message_type: MessageType::LocalRequest,
                            new_stock: None,
                            transactions: None,
                            orders: Some(vec![o]),
                        });
                    }
                    Command::SellFromFile(f) => {
                        info!(format!("New orders were issued. Info in: {:?}", f));
                        let orders = read_orders(f).await?;
                        store.do_send(StoreMessage {
                            message_type: MessageType::LocalRequest,
                            new_stock: None,
                            transactions: None,
                            orders: Some(orders),
                        });
                    }
                    Command::AddStock(o) => {
                        info!(format!("Adding new stock: {:?}", o));
                        let mut stock_to_add = Stock::new();
                        stock_to_add.insert(o.get_product(), o.get_qty());
                        store.do_send(StoreMessage {
                            message_type: MessageType::AddStock,
                            new_stock: Some(stock_to_add),
                            transactions: None,
                            orders: None,
                        });
                    }
                },
                Err(e) => {
                    error!(format!("The given command was not able to parse: {:?} ", e));
                    print_commands();
                }
            }
        }
    }

    let _ = cancel.clone();

    let _ = futures::future::join_all(t).await;
    let _ = updater.await;
    Ok(())
}

/// Awaits a message of start in order to handle the error in case they happen
fn start_listener(net: Addr<Listener>) -> JoinHandle<()> {
    tokio::spawn(async move {
        match net.send(ListenerState::Start).await {
            Ok(v) => match v {
                Ok(_) => {}
                Err(e) => {
                    error!(format!("There was an error in the TCP connection: {:?}", e))
                }
            },
            Err(e) => {
                error!(format!(
                    "There was an error while delivering the message to the listener Actor: {}",
                    e
                ))
            }
        }
    })
}

fn print_commands() {
    println!("The valid commands are:");
    println!("\t S | s For shutting down the whole application");
    println!("\t U | u To get the Network Up");
    println!("\t D | d For bringing down the Network");
    println!("\t O | o <product_id>,<quantity> For giving a new order to the system");
    println!("\t F | f <FilePath> For giving a new set of orders to the system. It will read the orders from the given filepath");
    println!("\t A | a <product_id>,<quantity> For adding stock to the system");
}

fn setup_updater(
    finish: CancellationToken,
    int_net: Arc<Mutex<NodeListener<ProtocolMessage<Stock, AbsoluteStateUpdate>, StoreGlue>>>,
    store: Addr<StoreActor>,
) -> JoinHandle<anyhow::Result<()>> {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_millis(1000)).await;
            select! {
                res = store.send(StoreState::CurrentState) => {
                    match res {
                        Ok(Some((id, info))) => {
                            let msg = transform_info(id,info);
                            let int_net = int_net.as_ref().lock().await;
                            let _ = int_net.broadcast(msg).await;
                        },
                        _ => {
                            println!("There was an error while getting the current state");
                        },
                    }
                }

                _ = finish.cancelled() => {
                    break;
                }
            }
        }

        Ok(())
    })
}

fn transform_info(
    id: NodeID,
    info: StoreInformation,
) -> ProtocolMessage<Stock, AbsoluteStateUpdate> {
    let message_type = ProtocolMessageType::Update;
    let stock_update = info.stock;
    let transaction_update = info.transactions.into_values().collect();

    ProtocolMessage {
        from: id,
        message_type,
        request_information: None,
        update_information: Some(AbsoluteStateUpdate {
            stock_update: Some(stock_update),
            transaction_update: Some(transaction_update),
        }),
    }
}
