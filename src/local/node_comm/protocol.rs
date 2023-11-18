use serde::{de::DeserializeOwned, Serialize};

pub enum ProtocolEvent {
    MaybeNew,
}

pub trait Operator<T>: Send + Sync
where
    T: Send + Serialize + DeserializeOwned,
{
    fn handle(&self, message: T) -> anyhow::Result<ProtocolEvent>;
}
