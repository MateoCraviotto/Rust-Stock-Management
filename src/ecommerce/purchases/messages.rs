use actix::Message;

use crate::common::order::Order;

pub type RequestID = u64;
pub type StoreID = u64;
pub enum MessageType {
    Update(StoreID),
    Request,
    Commit(RequestID),
    Cancel(RequestID),
}

pub enum RequestResponse {
    RequestOK(RequestID),
    RequestNOK
}

#[derive(Message)]
#[rtype(result = "Option<RequestResponse>")]
pub struct StoreMessage {
    message_type: MessageType,
    information: Vec<Order>
}