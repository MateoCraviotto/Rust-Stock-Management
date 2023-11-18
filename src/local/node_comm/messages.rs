use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub enum MessageType {
    Hello,
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
pub struct ProtocolMessage<T> {
    req_node_id: u64,
    res_node_id: u64,
    message_type: MessageType,
    request_information: Option<RequestInformation<T>>,
    update_information: Option<T>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RequestInformation<T> {
    request_id: u128,
    request_state: RequestState,
    information: Option<T>,
}
