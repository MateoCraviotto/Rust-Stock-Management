use actix::Message;

pub mod connection;
pub mod listen;

#[derive(Message)]
#[rtype(result = "anyhow::Result<()>")]
pub enum ListenerState {
    Start,
    Down,
    Shutdown,
}

enum CurrentState {
    Listening,
    Waiting,
}
