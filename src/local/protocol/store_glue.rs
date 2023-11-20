use actix::Addr;
use serde::{Deserialize, Serialize};

use crate::{
    ecommerce::purchases::store::{Stock, StoreActor, Transaction},
    local::{
        node_comm::{Operator, ProtocolEvent},
        NodeID,
    },
};

use super::messages::ProtocolMessage;

#[derive(Serialize, Deserialize, Debug)]
pub struct AbsoluteStateUpdate {
    stock_update: Option<Stock>,
    transaction_update: Option<Vec<Transaction>>,
}

type StoreMessage = ProtocolMessage<Stock, AbsoluteStateUpdate>;

pub struct StoreOperator {
    store: Addr<StoreActor>,
}

impl StoreOperator {
    pub fn new(store: Addr<StoreActor>) -> Self {
        Self { store }
    }
}

impl Operator<StoreMessage> for StoreOperator {
    fn handle(
        &self,
        from: NodeID,
        message: StoreMessage,
    ) -> anyhow::Result<ProtocolEvent<StoreMessage>> {
        todo!()
    }
}
