use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug)]
enum MessageType{
    Hello,
    Goodbye,
    Update,
    Request
}

#[derive(Serialize, Deserialize, Debug)]
enum RequestState{
    Ask,
    ResponseOK,
    ResponseNOK,
    Commit,
    Cancel
}

#[derive(Serialize, Deserialize, Debug)]
struct ProtocolMessage<T>{
    req_node_id: u64,
    res_node_id: u64, 
    message_type: MessageType,
    request_information: Option<RequestInformation<T>>,
    update_information: Option<T>
}

#[derive(Serialize, Deserialize, Debug)]
struct RequestInformation<T>{
    request_id: u128,
    request_state: RequestState,
    information: Option<T>
}

