use std::net::Ipv4Addr;
use std::str::FromStr;

use clap::{Parser, command, arg};

use crate::common::log::Verbosity;


#[derive(Parser, Debug)]
#[command(author, version, about, long_about=None)]
pub struct Args{
    /// Port of the local to connect to
    #[arg(short,long)]
    pub port: u16,

    /// Ip of the local to connect to
    #[arg(short, long, default_value_t = Ipv4Addr::from_str("127.0.0.1").unwrap())]
    pub ip: Ipv4Addr,

    /// Tells the level of verbosity
    #[arg(short, long, default_value_t = Verbosity::Debug)]
    pub verbosity: Verbosity 
}