use std::str::FromStr;

use actix::Addr;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    task::JoinHandle,
};

use crate::{
    ecommerce::{
        network::{listen::Listener, ListenerState},
        purchases::store::{Stock, StoreActor},
        sysctl::command::Command,
    },
    error, info,
    local::{
        node_comm::node_listener::NodeListener,
        protocol::{
            messages::ProtocolMessage,
            store_glue::{AbsoluteStateUpdate, StoreGlue},
        },
    },
};

pub async fn listen_commands(
    net: Addr<Listener>,
    mut int_net: NodeListener<ProtocolMessage<Stock, AbsoluteStateUpdate>, StoreGlue>,
    store: Addr<StoreActor>,
) -> anyhow::Result<()> {
    println!("Reading commands from STDIN");

    let mut lines = BufReader::new(tokio::io::stdin()).lines();

    // Save all the times that we tell the network to go up so we wait on all of them.
    // All of them should finish at the same time because of the cancellation tokens
    let mut t = vec![];
    t.push(start_listener(net.clone()));

    'main: loop {
        //let store_clone = store.clone();
        let r = lines.next_line().await;
        if let Ok(Some(line)) = r {
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
                        if !int_net.is_running() {
                            int_net = int_net.restart().await;
                        }
                        println!("Network was connected");
                    }
                    Command::NetDown => {
                        info!("Network Down order was given");
                        let _ = net.do_send(ListenerState::Down);
                        // Await all current messages to have a 'clean' environment
                        // in next iteration. Will block until all disconnections happened
                        let _ = futures::future::join_all(t).await;
                        t = vec![];

                        if int_net.is_running() {
                            int_net = int_net.shutdown().await;
                        }

                        println!("Network was disconnected");
                    }
                    Command::Sell(o) => {
                        info!(format!("New order was issued: {:?}", o));
                        //TODO: get the address of the actor that modifies things
                        //TODO: See how it modifies
                    }
                    Command::SellFromFile(f) => {
                        info!(format!("New orders were issued. Info in: {:?}", f));
                        //TODO: read file, read line by line, send to actor that modifies things
                    }
                    Command::AddStock(o) => {}
                },
                Err(e) => {
                    error!(format!("The given command was not able to parse: {:?} ", e));
                    print_commands();
                }
            }
        }
    }

    let _ = futures::future::join_all(t).await;
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
