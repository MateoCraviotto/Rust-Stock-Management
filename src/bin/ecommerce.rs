use std::time::Duration;

use clap::Parser;
use tokio::{io::AsyncWriteExt, net::TcpStream};
use tp::{ecommerce::{args::Args}, log_level, common::order::Order};

#[actix_rt::main]
async fn main() -> anyhow::Result<()>{
    let args = Args::parse();
    log_level!(args.verbosity);
    println!("Running E-Commerce which will connect to {}:{}", args.ip, args.port);

    let mut client = TcpStream::connect(format!("{}:{}", args.ip, args.port)).await?;
    loop{
        println!("Sending data to local");
        client.write_all("Sending this data\n".as_bytes()).await?;
        tokio::time::sleep(Duration::from_secs(10)).await;
    }

    Ok(())
}