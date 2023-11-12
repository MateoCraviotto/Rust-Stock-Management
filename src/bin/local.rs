use std::net::Ipv4Addr;

use actix::Actor;
use clap::Parser;
use tp::{local::args::Args, log_level, ecommerce::{sysctl::listener::listen_commands, network::listen::Listener}};

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
    let listener = Listener::new(ip, port).start();
    listen_commands(
        listener
    ).await
}