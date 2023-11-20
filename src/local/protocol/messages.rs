use crate::local::{NodeID, RequestID};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub enum MessageType {
    Goodbye,
    Update,
    Request,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum RequestState {
    Ask,
    ResponseOK,
    ResponseNOK,
    Commit,
    Cancel,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ProtocolMessage<M, S> {
    message_type: MessageType,
    request_information: Option<Request<M>>,
    update_information: Option<Vec<S>>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Request<M> {
    request_id: RequestID,
    requester: NodeID,
    request_state: RequestState,
    information: Option<NodeModification<M>>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct NodeModification<M> {
    affected: NodeID,
    modifications: Vec<M>,
}
