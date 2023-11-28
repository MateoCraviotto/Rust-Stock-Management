use std::collections::HashMap;

use actix::{Actor, Context, Handler};
use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::debug;

use super::messages::{MessageType, RequestID, StoreID, StoreMessage, StoreState};

pub type Stock = HashMap<u64, u64>;

#[derive(Clone, Debug)]
pub struct StoreInformation {
    pub stock: Stock,
    pub transactions: HashMap<RequestID, Transaction>,
    is_online: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum TransactionState {
    Cancelled,
    AwaitingConfirmation,
    NodeConfirmed,
    Finalized,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Transaction {
    pub id: RequestID,
    pub state: TransactionState,
    pub involved_stock: HashMap<StoreID, Stock>,
}

impl StoreInformation {
    fn random() -> Self {
        let stock = HashMap::new();

        Self {
            stock,
            is_online: false,
            transactions: HashMap::new(),
        }
    }

    fn is_online(&mut self, is_online: bool) {
        self.is_online = is_online
    }
}

pub struct StoreActor {
    stores: HashMap<StoreID, StoreInformation>,
    self_id: StoreID,
}

impl StoreActor {
    pub fn new(id: StoreID) -> Self {
        let self_info = Self::create_info();

        let mut stores_info = HashMap::new();

        stores_info.insert(id, self_info);

        Self {
            stores: stores_info,
            self_id: id,
        }
    }

    fn create_info() -> StoreInformation {
        let mut info = StoreInformation::random();

        info.is_online(true);

        info
    }
}

impl Actor for StoreActor {
    type Context = Context<Self>;
}

impl Handler<StoreMessage> for StoreActor {
    type Result = Option<Transaction>;

    fn handle(&mut self, msg: StoreMessage, _ctx: &mut Self::Context) -> Self::Result {
        match msg {
            StoreMessage {
                message_type: MessageType::Update(store_id),
                new_stock,
                transactions,
                orders: _,
            } => {
                let info = self.stores.get_mut(&store_id);
                match info {
                    Some(info) => {
                        if let Some(s) = new_stock {
                            info.stock = s;
                        }
                        if let Some(t) = transactions {
                            info.transactions = t;
                        }

                        info.is_online(true);
                    }
                    None => {
                        let mut info = Self::create_info();
                        if let Some(s) = new_stock {
                            info.stock = s;
                        }
                        if let Some(t) = transactions {
                            info.transactions = t;
                        }
                        info.is_online(true);
                        self.stores.insert(store_id, info);
                    }
                }

                None
            }
            StoreMessage {
                message_type: MessageType::Request,
                new_stock: _,
                transactions: _,
                orders,
            } => {
                let mut rng = rand::thread_rng();
                let new_transaction_id: RequestID = rng.gen();
                let stores_clone = self.stores.clone();
                let self_info = stores_clone.get(&self.self_id).cloned();
                match self_info {
                    Some(mut self_info) => {
                        let orders = match orders {
                            Some(orders) => orders,
                            None => {
                                return None;
                            }
                        };
                        // Check which orders I can complete (local_orders)
                        // Send the rest to other node(s)
                        let mut involved_stock: HashMap<StoreID, Stock> = HashMap::new();
                        let mut local_stock: Stock = Stock::new();
                        let mut remote_stock: Stock = Stock::new();
                        for order in orders {
                            let product = order.get_product();
                            let qty = order.get_qty();

                            if self_info.stock.contains_key(&product) {
                                let current_qty = self_info.stock[&product];
                                if qty > current_qty {
                                    if current_qty != 0 {
                                        // Use all I have
                                        local_stock.insert(product, current_qty);
                                        self_info.stock.insert(product, 0);
                                    }
                                    // Leave the rest to the other stores
                                    remote_stock.insert(product, qty - current_qty);
                                } else {
                                    // I have enough stock for that order
                                    local_stock.insert(product, qty);
                                    self_info.stock.insert(product, current_qty - qty);
                                }
                                involved_stock.insert(self.self_id, local_stock.clone());
                            } else {
                                remote_stock.insert(product, qty);
                            }
                        }

                        debug!(format!("Local stock {:?}", local_stock.clone()));
                        debug!(format!("Remote stock {:?}", remote_stock.clone()));
                        debug!(format!("Stores: {:?}", stores_clone.keys()));

                        let able_remote: Vec<u64> = self
                            .stores
                            .iter()
                            .filter_map(|(store_id, info)| {
                                if *store_id == self.self_id {
                                    return None;
                                }
                                if !info.is_online {
                                    return None;
                                }

                                Some(*store_id)
                            })
                            .collect();

                        // Check other nodes for remaining stock in remote_stock
                        // If there is enough stock, reserve it and add it to involved_stock
                        if !remote_stock.is_empty() {
                            for store_id in &able_remote {
                                let store_info = self.stores.get(store_id).cloned();
                                debug!(format!("Store info: {:?}", store_info));
                                match store_info {
                                    Some(mut store_info) => {
                                        println!(
                                            "Store {} has stock {:?}",
                                            store_id, store_info.stock
                                        );
                                        let mut store_stock = Stock::new();
                                        let remote_stock_clone = remote_stock.clone();
                                        for (product, qty) in remote_stock_clone {
                                            if store_info.stock.contains_key(&product) {
                                                let current_qty = store_info.stock[&product];
                                                if current_qty >= qty {
                                                    store_stock.insert(product, qty); // Reserve node stock
                                                    store_info
                                                        .stock
                                                        .insert(product, current_qty - qty); // Update node stock
                                                    remote_stock.remove(&product);
                                                }
                                            }
                                        }
                                        self.stores.insert(*store_id, store_info);
                                        involved_stock.insert(*store_id, store_stock);
                                    }
                                    None => {
                                        continue;
                                    }
                                }
                            }
                        }

                        // If there is still stock in remote_stock, cancel the transaction
                        if !remote_stock.is_empty() {
                            return Some(Transaction {
                                id: new_transaction_id,
                                state: TransactionState::Cancelled,
                                involved_stock,
                            });
                        }

                        let transaction: Transaction = Transaction {
                            id: new_transaction_id,
                            state: TransactionState::AwaitingConfirmation,
                            involved_stock,
                        };
                        self_info
                            .transactions
                            .insert(new_transaction_id, transaction.clone());
                        self.stores.insert(self.self_id, self_info);

                        Some(transaction)
                    }
                    None => None,
                }
            }
            StoreMessage {
                message_type: MessageType::Commit(request_id),
                new_stock: _,
                transactions: _,
                orders: _,
            } => {
                let self_info = self.stores.get_mut(&self.self_id)?;
                println!("Transactions: {:?}", self_info.transactions);
                let transaction = self_info.transactions.get(&request_id).cloned();
                match transaction {
                    Some(mut transaction) => {
                        transaction.state = TransactionState::Finalized;
                        self_info
                            .transactions
                            .insert(request_id, transaction.clone());
                        Some(transaction)
                    }
                    None => None,
                }
            }

            StoreMessage {
                message_type: MessageType::Cancel(request_id),
                new_stock: _,
                transactions: _,
                orders: _,
            } => {
                let stores_clone = self.stores.clone();
                let mut self_info = stores_clone.get(&self.self_id).cloned()?;
                println!("Transactions: {:?}", self_info.transactions);
                let transaction = self_info.transactions.get(&request_id).cloned();
                let mut transaction = match transaction {
                    Some(transaction) => transaction,
                    None => {
                        return None;
                    }
                };
                // Return stock
                for (store_id, stock) in &transaction.involved_stock {
                    let store_info = self.stores.get_mut(store_id);
                    match store_info {
                        Some(store_info) => {
                            for (product, qty) in stock {
                                let current_qty = store_info.stock[&product];
                                store_info.stock.insert(*product, current_qty + qty);
                            }
                        }
                        None => {
                            continue;
                        }
                    }
                }
                transaction.state = TransactionState::Cancelled;
                self_info
                    .transactions
                    .insert(request_id, transaction.clone());
                // Add changes in the cloned transaction to the store
                self.stores.insert(self.self_id, self_info.clone());
                Some(transaction)
            }
            StoreMessage {
                message_type: MessageType::LocalRequest,
                new_stock: _,
                transactions: _,
                orders,
            } => {
                let stores_clone = self.stores.clone();
                let self_info = stores_clone.get(&self.self_id).cloned();
                let mut self_info = match self_info {
                    Some(self_info) => self_info,
                    None => {
                        return None;
                    }
                };
                let orders = match orders {
                    Some(orders) => orders,
                    None => {
                        return None;
                    }
                };
                // Check which orders I can complete (local_orders)
                // If I cannot complete one order, abort the operation (None)
                let mut local_stock: Stock = Stock::new();
                for order in orders {
                    let product = order.get_product();
                    let qty = order.get_qty();

                    if self_info.stock.contains_key(&product) {
                        let current_qty = self_info.stock[&product];
                        if current_qty >= qty {
                            // Update local stock
                            local_stock.insert(product, qty); // Reserve local stock
                        } else {
                            println!(
                                "Not enough stock of product {}. Current stock is {}",
                                product, current_qty
                            );
                            return None;
                        }
                    } else {
                        println!(
                            "Not enough stock of product {}. Current stock is 0.",
                            product
                        );
                        return None;
                    }
                }
                // Update local stock
                for (product, qty) in local_stock.clone() {
                    let current_qty = self_info.stock[&product];
                    self_info.stock.insert(product, current_qty - qty);
                }
                self.stores.insert(self.self_id, self_info);
                let mut involved_stock: HashMap<u64, Stock> = HashMap::new();
                involved_stock.insert(self.self_id, local_stock);

                println!("Updated stock: {:?}", self.stores[&self.self_id].stock);

                Some(Transaction {
                    id: 0,
                    state: TransactionState::Finalized,
                    involved_stock,
                })
            }
            StoreMessage {
                message_type: MessageType::AddStock,
                new_stock,
                transactions: _,
                orders: _,
            } => {
                let stores_clone = self.stores.clone();
                let mut self_info = stores_clone.get(&self.self_id).cloned()?;
                let new_stock = match new_stock {
                    Some(new_stock) => new_stock,
                    None => {
                        return None;
                    }
                };
                for (product, qty) in new_stock {
                    if self_info.stock.contains_key(&product) {
                        let current_qty = self_info.stock[&product];
                        self_info.stock.insert(product, current_qty + qty);
                    } else {
                        self_info.stock.insert(product, qty);
                    }
                }
                println!("Current stock: {:?}", self_info.stock);
                self.stores.insert(self.self_id, self_info);
                None
            }
            StoreMessage {
                message_type: MessageType::Ask(request_id),
                new_stock: _,
                transactions: _,
                orders,
            } => {
                let stores_clone = self.stores.clone();
                let self_info = stores_clone.get(&self.self_id).cloned();
                let mut self_info = match self_info {
                    Some(self_info) => self_info,
                    None => {
                        return None;
                    }
                };
                /*let new_stock = match new_stock {
                    Some(new_stock) => new_stock,
                    None => {
                        return None;
                    }
                };*/
                let orders = match orders {
                    Some(orders) => orders,
                    None => {
                        return None;
                    }
                };
                // Check if I have every single product
                for order in orders.clone() {
                    if self_info.stock.contains_key(&order.get_product()) {
                        let current_qty = self_info.stock[&order.get_product()];
                        if current_qty < order.get_qty() {
                            return None;
                        }
                    } else {
                        return None;
                    }
                }
                // Update my stock
                for order in orders.clone() {
                    let current_qty = self_info.stock[&order.get_product()];
                    self_info
                        .stock
                        .insert(order.get_product(), current_qty - order.get_qty());
                }
                self.stores.insert(self.self_id, self_info);

                let transaction = Transaction {
                    id: request_id,
                    state: TransactionState::AwaitingConfirmation,
                    involved_stock: HashMap::new(),
                };
                // todo check if I should save the transaction with this local stock
                if let Some(me) = self.stores.get_mut(&self.self_id) {
                    me.transactions.insert(request_id, transaction.clone());
                }
                Some(transaction)
            }
            StoreMessage {
                message_type: MessageType::Confirm(request_id),
                new_stock: _,
                transactions: _,
                orders: _,
            } => {
                let stores_clone = self.stores.clone();
                let self_info = stores_clone.get(&self.self_id).cloned();
                let mut self_info = match self_info {
                    Some(self_info) => self_info,
                    None => {
                        return None;
                    }
                };
                let transaction = self_info.transactions.get(&request_id).cloned();
                let mut transaction = match transaction {
                    Some(transaction) => transaction,
                    None => {
                        return None;
                    }
                };
                transaction.state = TransactionState::NodeConfirmed;
                self_info
                    .transactions
                    .insert(request_id, transaction.clone());
                // Add changes in the cloned transaction to the store
                self.stores.insert(self.self_id, self_info.clone());
                None
            }
        }
    }
}

impl Handler<StoreState> for StoreActor {
    type Result = Option<(StoreID, StoreInformation)>;

    fn handle(&mut self, msg: StoreState, _ctx: &mut Self::Context) -> Self::Result {
        match msg {
            StoreState::CurrentState => self
                .stores
                .get(&self.self_id)
                .and_then(|info| Some((self.self_id, info.clone()))),
            StoreState::Shutdown(store_id) => self.stores.get_mut(&store_id).and_then(|store| {
                store.is_online(false);
                None
            }),
        }
    }
}
