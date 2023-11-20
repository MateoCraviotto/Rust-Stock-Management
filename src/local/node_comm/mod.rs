use super::NodeID;

pub mod node_communication;
pub mod node_listener;

pub enum ProtocolEvent<T> {
    Nothing,
    MaybeNew,
    Response(T),
    Teardown,
}
