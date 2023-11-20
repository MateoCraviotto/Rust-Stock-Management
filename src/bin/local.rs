use std::net::Ipv4Addr;

use actix::Actor;
use clap::Parser;
use tp::{
    ecommerce::{
        network::listen::Listener, purchases::store::StoreActor, sysctl::listener::listen_commands,
    },
    local::{args::Args, node_comm::node_listener::NodeListener, protocol::store_glue::StoreGlue},
    log_level,
};

#[actix_rt::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    log_level!(args.verbosity);

    println!(
        "Starting the Local process in address: {}:{}",
        args.ip, args.extern_port
    );
    println!(
        "Starting the inter-node communication in address: {}:{}. Should connect to: {:?}",
        args.ip, args.intern_port, args.node_ports
    );

    let result = start(
        args.store_id,
        args.ip,
        args.extern_port,
        args.intern_port,
        args.node_ports,
    )
    .await;

    match &result {
        Ok(_) => {
            println!("Goodbye :)");
        }
        Err(_) => {
            println!("There was an error while running the program");
        }
    };

    return result;
}

async fn start(
    me: u64,
    ip: Ipv4Addr,
    external_port: u16,
    internal_port: u16,
    internal_port_list: Vec<u16>,
) -> anyhow::Result<()> {
    let store = StoreActor::new(me).start();
    let listener = Listener::new(ip, external_port, store.clone()).start();
    let store_glue = StoreGlue::new(me, store.clone()).start();
    let internal_listener =
        NodeListener::start(internal_port, ip, me, internal_port_list, store_glue);
    listen_commands(listener, internal_listener, store).await
}
