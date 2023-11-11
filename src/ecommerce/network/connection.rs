use actix::{Actor, Context, Handler, ResponseActFuture};
use tokio::net::TcpStream;

use super::ListenerState;

pub struct Communication{
    stream: TcpStream,
}

impl Communication{
    pub fn new(stream: TcpStream) -> Self{
        Self { stream }
    }
}

impl Actor for Communication{
    type Context = Context<Self>;
}

impl Handler<ListenerState> for Communication{
    type Result = ResponseActFuture<Self, anyhow::Result<()>>;

    fn handle(&mut self, msg: ListenerState, ctx: &mut Self::Context) -> Self::Result {
        todo!()
    }
}