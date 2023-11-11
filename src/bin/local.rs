use clap::Parser;
use tp::{local::args::Args, log_level};

#[actix_rt::main]
async fn main(){
    let args = Args::parse();
    log_level!(args.verbosity);
    
    println!("Starting the Local process in port in address: {}:{}", args.ip, args.port);
}