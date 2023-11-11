use std::net::Ipv4Addr;
use std::str::FromStr;

use clap::{Parser, command, arg};

use crate::common::log::Verbosity;


#[derive(Parser, Debug)]
#[command(author, version, about, long_about=None)]
pub struct Args{
    /// Port where to start to listen for e-commerce applications
    #[arg(short,long)]
    pub port: u16,

    /// Ip where to bind the e-commerce listener
    #[arg(short, long, default_value_t = Ipv4Addr::from_str("127.0.0.1").unwrap())]
    pub ip: Ipv4Addr,

    /// Tells the level of verbosity
    #[arg(short, long, default_value_t = Verbosity::Debug)]
    pub verbosity: Verbosity 
}