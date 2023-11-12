use std::net::Ipv4Addr;

use actix::{Actor, Context, Handler, ResponseActFuture, WrapFuture, ActorFutureExt, Addr};
use tokio::{net::TcpListener, select};
use tokio_util::sync::CancellationToken;

use crate::info;

use super::{CurrentState, ListenerState, connection::Communication};

pub struct Listener{
    current_state: CurrentState,
    port: u16,
    ip: Ipv4Addr,
    cancel_token: CancellationToken,
}

impl Listener{
    pub fn new(ip: Ipv4Addr, port: u16) -> Self{
        Listener { 
            current_state: CurrentState::Waiting,
            cancel_token: CancellationToken::new(),
            port, 
            ip, 
        }
    }
}

impl Actor for Listener{
    type Context = Context<Self>;
}

impl Handler<ListenerState> for Listener{
    type Result = ResponseActFuture<Self, anyhow::Result<()>>;

    fn handle(&mut self, msg: ListenerState, _ctx: &mut Self::Context) -> Self::Result {
        match (msg, &self.current_state){
            (ListenerState::Start, CurrentState::Waiting) => {
                let cancellation = self.cancel_token.clone();
                let to = format!("{}:{}", self.ip, self.port);
                self.current_state = CurrentState::Listening;
                Box::pin(start_listening(to, cancellation)
                    .into_actor(self)
                    .map(move |result, me, _ctx|{
                        me.current_state = CurrentState::Waiting;
                        return result;
                    })
                )
            },
            (ListenerState::Down | ListenerState::Shutdown, CurrentState::Listening) => {
                let cancellation = self.cancel_token.clone();
                cancellation.cancel();
                Box::pin(async {}
                    .into_actor(self)
                    .map(move |_result, _me, _ctx|{
                        return Ok(());
                    })
                )
            },
            _ => {
                Box::pin(async {}
                    .into_actor(self)
                    .map(move |_result, _me, _ctx|{
                        return Ok(());
                    })
                )
            }
        }
    }
}

async fn start_listening(to: String, cancel: CancellationToken) -> anyhow::Result<()>{
    let listener = TcpListener::bind(&to).await?;
    let mut connected: Vec<Addr<Communication>> = vec![];

    'accept: loop{
        info!(format!("Listening in {}", &to));
        select! {
            conn_result = listener.accept() => {
                let (stream, addr) = conn_result?;
                info!(format!("New connection for {}", addr));
                connected.push(Communication::new(stream).start())
            }

            _ = cancel.cancelled() => {
                info!("Got order to take down the TCP stream. Sending message to shutdown all child TCP streams");
                for con in connected{
                    let _ = con.do_send(ListenerState::Shutdown);
                }
                break 'accept;
            }
        }
    }

    Ok(())
}

