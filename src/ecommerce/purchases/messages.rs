use crate::common::order::Order;
use actix::Message;
use std::collections::HashMap;

use super::store::{Stock, Transaction};

pub type RequestID = u128;
pub type StoreID = u64;
pub enum MessageType {
    Update(StoreID),
    Request,
    Ask(RequestID),
    Confirm(RequestID),
    Commit(RequestID),
    Cancel(RequestID),
    LocalRequest,
    AddStock,
}

pub enum RequestResponse {
    RequestOK(RequestID),
    RequestNOK,
}

#[derive(Message)]
#[rtype(result = "Option<Transaction>")]
pub struct StoreMessage {
    pub message_type: MessageType,
    pub new_stock: Option<Stock>,
    pub transactions: Option<HashMap<RequestID, Transaction>>,
    pub orders: Option<Vec<Order>>,
}
