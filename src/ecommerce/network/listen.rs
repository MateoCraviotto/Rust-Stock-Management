use std::{net::Ipv4Addr, sync::Arc};

use actix::{Actor, ActorFutureExt, Context, Handler, ResponseActFuture, WrapFuture};
use tokio::{net::TcpListener, select, sync::Mutex};
use tokio_util::sync::CancellationToken;

use crate::{debug, ecommerce::purchases::store::Store, info};

use super::{connection::Communication, CurrentState, ListenerState};

pub struct Listener {
    current_state: CurrentState,
    port: u16,
    ip: Ipv4Addr,
    cancel_token: CancellationToken,
    store: Arc<Mutex<Store>>,
}

impl Listener {
    pub fn new(ip: Ipv4Addr, port: u16, store: Arc<Mutex<Store>>) -> Self {
        Listener {
            current_state: CurrentState::Waiting,
            cancel_token: CancellationToken::new(),
            port,
            ip,
            store,
        }
    }
    pub fn store(&mut self) -> Arc<Mutex<Store>> {
        self.store.clone()
    }
}

impl Actor for Listener {
    type Context = Context<Self>;
}

impl Handler<ListenerState> for Listener {
    type Result = ResponseActFuture<Self, anyhow::Result<()>>;

    /// Handles the lifecycle of the TCP Listener
    ///
    /// | State/Message | Stop/Shutdown                                                                         | Listen                                                                                                                                                                  |
    /// |---------------|---------------------------------------------------------------------------------------|-------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
    /// | Listening     | Send a cancel order to the listen task. Will cause the return of the listening future | Nothing                                                                                                                                                                 |
    /// | Waiting       | Nothing                                                                               | Start the listening.  Will return a future that returns when it finishes when it stops listening. The future will have an error in case that a network error happened.  |
    fn handle(&mut self, msg: ListenerState, _ctx: &mut Self::Context) -> Self::Result {
        match (msg, &self.current_state) {
            (ListenerState::Start, CurrentState::Waiting) => {
                self.cancel_token = CancellationToken::new();
                let cancellation = self.cancel_token.clone();
                let to = format!("{}:{}", self.ip, self.port);
                let a = to.clone();
                self.current_state = CurrentState::Listening;
                Box::pin(
                    start_listening(to, cancellation, self.store.clone())
                        .into_actor(self)
                        .map(move |result, me, _ctx| {
                            debug!(format!("Finishing the listening on {}", a));
                            me.current_state = CurrentState::Waiting;
                            return result;
                        }),
                )
            }
            (ListenerState::Down | ListenerState::Shutdown, CurrentState::Listening) => {
                let cancellation = self.cancel_token.clone();
                cancellation.cancel();
                debug!("Sending cancellation token for Listener");
                Box::pin(async {}.into_actor(self).map(move |_result, _me, _ctx| {
                    return Ok(());
                }))
            }
            _ => Box::pin(
                async {
                    debug!("Default do nothing");
                }
                .into_actor(self)
                .map(move |_result, _me, _ctx| {
                    return Ok(());
                }),
            ),
        }
    }
}

async fn start_listening(
    to: String,
    cancel: CancellationToken,
    store: Arc<Mutex<Store>>,
) -> anyhow::Result<()> {
    let listener = TcpListener::bind(&to).await?;
    let mut connected = vec![];
    let mut tasks = vec![];

    'accept: loop {
        info!(format!("Listening in {}", &to));
        select! {
            conn_result = listener.accept() => {
                let (stream, addr) = conn_result?;
                info!(format!("New connection for {}", addr));
                let (actor, task) = Communication::new(stream, store.clone());
                let new = actor.start();
                let _ = new.do_send(ListenerState::Start);
                tasks.push(task);
                connected.push(new);
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

    let _ = futures::future::join_all(tasks);

    Ok(())
}
