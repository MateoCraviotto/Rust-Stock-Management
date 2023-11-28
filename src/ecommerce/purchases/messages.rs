use crate::common::order::Order;
use crate::ecommerce::purchases::store::StoreInformation;
use actix::Message;
use anyhow::bail;
use std::{collections::HashMap, str::FromStr};

use super::store::{Stock, Transaction};

pub type RequestID = u128;
pub type StoreID = u64;

#[derive(Debug)]
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

impl FromStr for MessageType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.split_whitespace().collect::<Vec<&str>>()[..] {
            ["update", store_id] => Ok(MessageType::Update(u64::from_str(store_id)?)),
            ["request"] => Ok(MessageType::Request),
            ["ask", request_id] => Ok(MessageType::Ask(u128::from_str(request_id)?)),
            ["confirm", request_id] => Ok(MessageType::Confirm(u128::from_str(request_id)?)),
            ["commit", request_id] => Ok(MessageType::Commit(u128::from_str(request_id)?)),
            ["cancel", request_id] => Ok(MessageType::Cancel(u128::from_str(request_id)?)),
            ["local_request"] => Ok(MessageType::LocalRequest),
            ["add_stock"] => Ok(MessageType::AddStock),
            _ => bail!("Invalid message type"),
        }
    }
}

pub enum RequestResponse {
    RequestOK(RequestID),
    RequestNOK,
}

#[derive(Message, Debug)]
#[rtype(result = "Option<Transaction>")]
pub struct StoreMessage {
    pub message_type: MessageType,
    pub new_stock: Option<Stock>,
    pub transactions: Option<HashMap<RequestID, Transaction>>,
    pub orders: Option<Vec<Order>>,
}

#[derive(Message)]
#[rtype(result = "Option<(StoreID,StoreInformation)>")]
pub enum StoreState {
    CurrentState,
}
