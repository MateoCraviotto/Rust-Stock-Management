use std::{net::Ipv4Addr, sync::Arc};

use actix::Actor;
use clap::Parser;
use tokio::sync::Mutex;
use tp::{local::args::Args, log_level, ecommerce::{sysctl::listener::listen_commands, network::listen::Listener, purchases::store::Store}};

#[actix_rt::main]
async fn main() -> anyhow::Result<()>{
    let args = Args::parse();
    log_level!(args.verbosity);
    
    println!("Starting the Local process in port in address: {}:{}", args.ip, args.port);

    let result = start(args.ip, args.port).await;

    match &result{
        Ok(_) => {
            println!("Goodbye :)");
        },
        Err(_) => {
            println!("There was an error while running the program");
        },
    };

    return result;
}

async fn start(ip : Ipv4Addr, port: u16) -> anyhow::Result<()>{
    let store = Arc::new(Mutex::new(Store::new()));
    let listener = Listener::new(ip, port, store.clone()).start();
    store.as_ref().lock().await.add_to_network(listener.clone());
    listen_commands(
        listener,
        store
    ).await
}