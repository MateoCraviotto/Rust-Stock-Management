use std::collections::HashMap;

use actix::{Addr, Actor, Context, Handler};

use crate::{ecommerce::network::listen::Listener, error, info, common::order::Order};

use super::{purchase_state::PurchaseState, messages::{StoreMessage, RequestResponse, StoreID, RequestID}};

type Stock = HashMap<u64, u64>;

struct StoreInformation{
    stock: Stock,
    transactions: Transaction,
    is_online: bool
}

enum TransactionState{
    Cancelled,
    AwaitingConfirmation,
    NodeConfirmed,
    Finalized
}

struct Transaction{
    id: RequestID,
    state: TransactionState,
    involved_stock: Stock,
}

impl StoreInformation{
    fn random() -> Self{
        let stock = HashMap::new();
        
        Self { stock, is_online: false }
    }

    fn is_online(&mut self, is_online: bool) {
        self.is_online = is_online
    }
}

pub struct Store {
    //id: String,
    products: HashMap<u64, u64>,
    listener: Option<Addr<Listener>>,
}

impl Store {
    pub fn new() -> Self {
        Store {
            products: HashMap::new(),
            listener: None
        }
    }

    pub fn get_products(&self)-> HashMap<u64, u64> {
        self.products.clone()
    }

    pub fn get_product_quantity(&self, product: u64)-> u64{
        match self.products.get(&product){
            Some(qty) => *qty,
            None => {
                error!(format!("Product {} not found", product));
                0
            },
        }
    }

    pub fn listener(&self) -> Option<Addr<Listener>> {
        match &self.listener {
            Some(addr) => Some(addr.clone()),
            None => {
                error!("The store is disconnected");
                None
            },
        }
    }

    pub fn add_to_network(&mut self, listener: Addr<Listener>) {
        self.listener = Some(listener);
    }

    pub fn add_product_stock(&mut self, id: u64, quantity: u64) -> u64 {
        if !self.products.contains_key(&id) {
            self.products.insert(id, quantity);
        } else {
            let current_amount = self.products[&id];
            self.products.insert(id, current_amount + quantity);
        }
        self.products[&id]
    }

    pub fn sell_product(&mut self, id: u64, quantity: u64) {
        let mut remainder = self.products[&id];
        if remainder >= quantity {
            remainder -= quantity;
            self.products.insert(id,remainder);
            info!(format!("Selling {} of product {}", quantity, id));
        } else {
            error!("Not enough products to sell");
        }
    }

    /// Manages incoming order
    pub fn manage_order(&mut self, order: Order) -> PurchaseState {
        let id = order.get_product();
        let qty = order.get_qty();
        println!("Current products: {:?}", self.get_products());
        if self.products.contains_key(&id) {
            let current_quantity = self.products[&id];
            if current_quantity >= qty {
                self.sell_product(id, qty);
                PurchaseState::Reserve
            } else {
                PurchaseState::Cancel
            }
        } else {
            PurchaseState::Cancel
        }
    }

    pub fn clone(&self) -> Self {
        Store {
            //id: self.id.clone(),
            products: self.products.clone(),
            listener: match self.listener {
                Some(ref addr) => Some(addr.clone()),
                None => None,
            }
        }
    }
}


pub struct StoreActor{
    stores: HashMap<StoreID, StoreInformation>,
    self_id: StoreID
}

impl StoreActor{
    pub fn new(id: StoreID)->Self{
        let self_info = Self::create_info();

        let mut stores_info = HashMap::new();

        stores_info.insert(id, self_info);

        Self{
            stores: stores_info,
            self_id: id
        }
    }

    fn create_info() -> StoreInformation {
        let mut info = StoreInformation::random();

        info.is_online(true);

        return info;
    }
}

impl Actor for StoreActor{
    type Context = Context<Self>;
}

impl Handler<StoreMessage> for StoreActor{
    type Result = Option<RequestResponse>;

    fn handle(&mut self, msg: StoreMessage, ctx: &mut Self::Context) -> Self::Result {
        todo!()
    }
}