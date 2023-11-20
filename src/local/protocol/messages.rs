use crate::local::node_comm::ProtocolEvent;
use crate::local::protocol::store_glue::ProtocolStoreMessage;
use crate::local::{NodeID, RequestID};
use actix::Message;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Message)]
#[rtype(result = "anyhow::Result<ProtocolEvent<ProtocolStoreMessage>>")]
pub enum ProtocolMessageType {
    Goodbye,
    Update,
    Request,
}

#[derive(Serialize, Deserialize, Debug, Message)]
#[rtype(result = "anyhow::Result<ProtocolEvent<ProtocolStoreMessage>>")]

pub enum RequestAction {
    Ask,
    Confirm,
    Commit,
    Cancel,
}

#[derive(Serialize, Deserialize, Debug, Message)]
#[rtype(result = "anyhow::Result<ProtocolEvent<ProtocolStoreMessage>>")]

pub struct ProtocolMessage<M, S> {
    pub from: NodeID,
    pub message_type: ProtocolMessageType,
    pub request_information: Option<Request<M>>,
    pub update_information: Option<S>,
}

#[derive(Serialize, Deserialize, Debug, Message)]
#[rtype(result = "anyhow::Result<ProtocolEvent<ProtocolStoreMessage>>")]

pub struct Request<M> {
    pub request_id: RequestID,
    pub requester: NodeID,
    pub request_state: RequestAction,
    pub information: Option<Vec<NodeModification<M>>>,
}

#[derive(Serialize, Deserialize, Debug, Message)]
#[rtype(result = "anyhow::Result<ProtocolEvent<ProtocolStoreMessage>>")]

pub struct NodeModification<M> {
    pub affected: NodeID,
    pub modifications: Vec<M>,
}
