use std::str::FromStr;

use actix::Addr;
use tokio::{io::{BufReader, AsyncBufReadExt}, task::JoinHandle};

use crate::{ecommerce::{sysctl::command::Command, network::{listen::Listener, ListenerState}}, error, info};


pub async fn listen_commands(net: Addr<Listener>) -> anyhow::Result<()> {
    println!("Reading commands from STDIN");

    let mut lines = BufReader::new(tokio::io::stdin()).lines();

    // Save all the times that we tell the network to go up so we wait on all of them.
    // All of them should finish at the same time because of the cancellation tokens
    let mut t = vec![];
    t.push(start_listener(net.clone()));

    'main: loop {
        let r = lines.next_line().await;
        if let Ok(Some(line)) = r{
            match Command::from_str(&line){
                Ok(c) => match c {
                    Command::Shutdown => {
                        info!("Shutdown order was given");
                        net.do_send(ListenerState::Shutdown);
                        break 'main;
                    },
                    Command::NetUp => {
                        info!("Network Down order was given");
                        t.push(start_listener(net.clone()));
                    },
                    Command::NetDown => {
                        info!("Network Up order was given");
                        let _ = net.do_send(ListenerState::Down);
                    },
                    Command::Sell(o) => {
                        info!(format!("New order was issued: {:?}", o));
                        //TODO: get the address of the actor that modifies things
                        //TODO: See how it modifies
                    },
                    Command::SellFromFile(f) => {
                        info!(format!("New orders were issued. Info in: {:?}", f));
                        //TODO: read file, read line by line, send to actor that modifies things
                    },
                },
                Err(e) => {
                    error!(format!("The given command was not able to parse: {:?} ", e));
                    print_commands();
                },
            }
        }
    }

    let _ = futures::future::join_all(t).await;

    Ok(())
}

fn start_listener(net: Addr<Listener>) -> JoinHandle<()>{
    tokio::spawn(async move{
        match net.send(ListenerState::Start).await {
            Ok(v) => {
                match v{
                    Ok(_) => {
                        info!("TCP Listener gracefully shot down");
                    },
                    Err(e) => {
                        error!(format!("There was an error in the TCP connection: {:?}", e))
                    },
                }
            },
            Err(e) => {
                error!(format!("There was an error while delivering the message to the listener Actor: {}", e))
            },
        }
    })
}

fn print_commands(){
    println!("The valid commands are:");
}