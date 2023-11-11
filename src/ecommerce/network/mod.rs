use actix::Message;

pub mod listen;
pub mod connection;

#[derive(Message)]
#[rtype(result = "anyhow::Result<()>")]
pub enum ListenerState {
    Start,
    Down,
    Shutdown
}

enum CurrentState {
    Listening,
    Waiting
}