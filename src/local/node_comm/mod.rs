use actix::Message;

use super::NodeID;

pub mod node_communication;
pub mod node_listener;

#[derive(Debug)]
pub enum ProtocolEvent<T> {
    Nothing,
    MaybeNew,
    Response(T),
    Teardown,
}

#[derive(Message, Debug)]
#[rtype(result = "()")]
pub enum ActorLifetime {
    Shutdown(NodeID),
}
