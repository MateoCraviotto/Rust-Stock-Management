use std::collections::HashMap;

use actix::Addr;

use crate::{ecommerce::network::listen::Listener, error, info, common::order::Order};

use super::purchase_state::PurchaseState;


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
