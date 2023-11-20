use serde::{de::DeserializeOwned, Serialize};

use super::NodeID;

pub mod node_communication;
pub mod node_listener;

pub enum ProtocolEvent<T> {
    MaybeNew,
    Response(T),
    Teardown,
}

pub trait Operator<T>: Send + Sync
where
    T: Send + Serialize + DeserializeOwned,
{
    fn handle(&self, from: NodeID, message: T) -> anyhow::Result<ProtocolEvent<T>>;
}
