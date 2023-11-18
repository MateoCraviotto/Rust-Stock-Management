use std::collections::HashMap;
use actix::Message;
use crate::common::order::Order;

use super::store::{Transaction, Stock};

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
#[rtype(result = "Option<Transaction>")]
pub struct StoreMessage {
    pub message_type: MessageType,
    pub new_stock: Option<Stock>,
    pub transactions: Option<HashMap<RequestID, Transaction>>,
    pub orders: Option<Vec<Order>>,
}
