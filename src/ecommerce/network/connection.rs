
use actix::{Actor, Context, Handler, ResponseActFuture, WrapFuture, ActorFutureExt};
use tokio::{net::TcpStream, io::{BufReader, BufWriter, AsyncBufReadExt}, task::JoinHandle, select};
use tokio_util::sync::CancellationToken;

use crate::{info, debug};

use super::ListenerState;

pub struct Communication{
    cancel_token: CancellationToken
}

impl Communication{
    pub fn new(stream: TcpStream) -> (Self, JoinHandle<anyhow::Result<()>>){
        let mine = CancellationToken::new();
        let pair = mine.clone();

        let handle = tokio::spawn(serve(stream, pair));
        
        (
            Self { 
                cancel_token: mine
            }, 
            handle
        )
    }
}

impl Actor for Communication{
    type Context = Context<Self>;
}

impl Handler<ListenerState> for Communication{
    type Result = ResponseActFuture<Self, anyhow::Result<()>>;

    fn handle(&mut self, msg: ListenerState, _ctx: &mut Self::Context) -> Self::Result {
        match msg{
            ListenerState::Down | ListenerState::Shutdown => {
                self.cancel_token.cancel();
                debug!(format!("Sending cancellation token for connection, {:?}", self.cancel_token));
                Box::pin(async {
                }
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

async fn serve(stream: TcpStream, token: CancellationToken) -> anyhow::Result<()> {
    let (read, mut write) = stream.into_split();

    let mut reader = BufReader::new(read);
    let mut writer = BufWriter::new(write);
    let mut data = String::new();

    'serving: loop{
        debug!(format!("Waiting for data or connection or cancellation. {:?}", token));
        select! {
            r = reader.read_line(&mut data) => {
                debug!("Read some data");
                if r? == 0{
                    break 'serving;
                }
                println!("RECEIVED: {}", data);
                data = String::new();
            }

            _ = token.cancelled() => {
                info!("Closing a connection");
                break 'serving;
            }
        }
    }

    Ok(())
}