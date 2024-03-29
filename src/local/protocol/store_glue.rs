use std::collections::HashMap;

use actix::{
    Actor, ActorFutureExt, Addr, Context, Handler, Message, ResponseActFuture, WrapFuture,
};
use anyhow::bail;
use serde::{Deserialize, Serialize};

use crate::common::order::Order;
use crate::ecommerce::purchases::messages::{MessageType, StoreID, StoreMessage, StoreState};
use crate::ecommerce::purchases::store::{StoreActor, Transaction};
use crate::local::node_comm::ActorLifetime;
use crate::local::protocol::messages::Request;
use crate::local::{NodeID, RequestID};
use crate::{ecommerce::purchases::store::Stock, local::node_comm::ProtocolEvent};

use super::messages::{ProtocolMessage, ProtocolMessageType, RequestAction};

#[derive(Serialize, Deserialize, Debug, Message, Clone)]
#[rtype(result = "anyhow::Result<ProtocolEvent<ProtocolStoreMessage>>")]
pub struct AbsoluteStateUpdate {
    pub stock_update: Option<Stock>,
    pub transaction_update: Option<Vec<Transaction>>,
}

pub type ProtocolStoreMessage = ProtocolMessage<Stock, AbsoluteStateUpdate>;

#[derive(Clone)]
pub struct StoreGlue {
    me: StoreID,
    store: Addr<StoreActor>,
}

impl StoreGlue {
    pub fn new(me: StoreID, store: Addr<StoreActor>) -> Self {
        Self { me, store }
    }
}

impl StoreGlue {
    fn transform_update(from: NodeID, message: ProtocolStoreMessage) -> Option<StoreMessage> {
        let message_type = MessageType::Update(from);

        let (stock, transactions) = message
            .update_information
            .map(|updates| (updates.stock_update, updates.transaction_update))?;

        let new_stock = stock;

        let transactions = transactions.map(|transactions| {
            transactions
                .into_iter()
                .map(|t| (t.id, t))
                .collect::<HashMap<_, _>>()
        });

        Some(StoreMessage {
            message_type,
            new_stock,
            transactions,
            orders: None,
        })
    }

    fn transform_request(
        me: StoreID,
        message: ProtocolStoreMessage,
    ) -> Option<(RequestID, StoreMessage)> {
        let (id, state, information) = message
            .request_information
            .map(|info| (info.request_id, info.request_state, info.information))?;

        let message_type = Self::get_transaction_type(id, state);

        let orders = information?
            .into_iter()
            .filter_map(|node_modif| {
                if node_modif.affected == me {
                    Some(node_modif.modifications)
                } else {
                    None
                }
            })
            .flatten()
            .reduce(|mut acc, e| {
                e.into_iter().for_each(|(product, amount)| {
                    let new_amount = acc.get(&product).unwrap_or(&0) + amount;
                    acc.insert(product, new_amount);
                });
                acc
            })?
            .into_iter()
            .map(|(product, amount)| Order::new(product, amount))
            .collect::<Vec<_>>();

        Some((
            id,
            StoreMessage {
                message_type,
                new_stock: None,
                transactions: None,
                orders: Some(orders),
            },
        ))
    }

    fn get_transaction_type(request: RequestID, action: RequestAction) -> MessageType {
        match action {
            RequestAction::Ask => MessageType::Ask(request),
            RequestAction::Confirm => MessageType::Confirm(request),
            RequestAction::Commit => MessageType::Commit(request),
            RequestAction::Cancel => MessageType::Cancel(request),
        }
    }
}

impl Actor for StoreGlue {
    type Context = Context<Self>;
}

impl Handler<ProtocolStoreMessage> for StoreGlue {
    type Result = ResponseActFuture<Self, anyhow::Result<ProtocolEvent<ProtocolStoreMessage>>>;

    fn handle(&mut self, msg: ProtocolStoreMessage, _ctx: &mut Self::Context) -> Self::Result {
        let store = self.store.clone();
        let me = self.me;
        let from = msg.from;
        Box::pin(
            async move {
                match msg.message_type {
                    ProtocolMessageType::Goodbye => {
                        Ok(ProtocolEvent::<ProtocolStoreMessage>::Teardown)
                    }
                    ProtocolMessageType::Update => {
                        if let Some(m) = Self::transform_update(msg.from, msg) {
                            store.do_send(m)
                        }
                        Ok(ProtocolEvent::MaybeNew)
                    }
                    ProtocolMessageType::Request => {
                        if let Some((req_id, m)) = Self::transform_request(me, msg) {
                            match m.message_type {
                                MessageType::Ask(_) => {
                                    match store.send(m).await {
                                        Ok(Some(_)) => {
                                            return Ok(ProtocolEvent::Response(
                                                ProtocolStoreMessage {
                                                    from: me,
                                                    message_type: ProtocolMessageType::Request,
                                                    request_information: Some(Request {
                                                        request_id: req_id,
                                                        requester: from,
                                                        request_state: RequestAction::Confirm,
                                                        information: None,
                                                    }),
                                                    update_information: None,
                                                },
                                            ))
                                        }
                                        Ok(None) => {
                                            return Ok(ProtocolEvent::Response(
                                                ProtocolStoreMessage {
                                                    from: me,
                                                    message_type: ProtocolMessageType::Request,
                                                    request_information: Some(Request {
                                                        request_id: req_id,
                                                        requester: from,
                                                        request_state: RequestAction::Cancel,
                                                        information: None,
                                                    }),
                                                    update_information: None,
                                                },
                                            ))
                                        }
                                        Err(_) => bail!("Node error"),
                                    };
                                }
                                _ => {
                                    let _ = store.send(m).await;
                                    return Ok(ProtocolEvent::Nothing);
                                }
                            }
                        }
                        bail!("Invalid request")
                    }
                }
            }
            .into_actor(self)
            .map(move |result, _me, _ctx| result),
        )
    }
}

impl Handler<ActorLifetime> for StoreGlue {
    type Result = ();

    fn handle(&mut self, msg: ActorLifetime, _ctx: &mut Self::Context) -> Self::Result {
        match msg {
            ActorLifetime::Shutdown(id) => {
                self.store.do_send(StoreState::Shutdown(id));
            }
        }
    }
}
