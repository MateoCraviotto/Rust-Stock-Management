use std::str::FromStr;

use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncBufReadExt;

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone, Copy)]
pub struct Order {
    product_id: u64,
    qty: u64,
}

impl Order {
    pub fn new<T>(product: T, qty: T) -> Self
    where
        T: Into<u64>,
    {
        Self {
            product_id: product.into(),
            qty: qty.into(),
        }
    }

    pub fn get_product(&self) -> u64 {
        self.product_id
    }

    pub fn get_qty(&self) -> u64 {
        self.qty
    }
}

impl ToString for Order {
    fn to_string(&self) -> String {
        format!("{},{}", self.product_id, self.qty)
    }
}

impl FromStr for Order {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let splitted: Vec<&str> = s.trim().split(",").collect();
        let product_id = splitted
            .get(0)
            .ok_or(anyhow!(
                "An order must be an ID and a QTY in the format <id: u64>,<qty: u64>"
            ))?
            .trim()
            .parse()?;
        let qty = splitted
            .get(1)
            .ok_or(anyhow!(
                "An order must be an ID and a QTY in the format <id: u64>,<qty: u64>"
            ))?
            .trim()
            .parse()?;

        Ok(Order { product_id, qty })
    }
}

pub async fn read_orders(filename: String) -> anyhow::Result<Vec<Order>> {
    let mut orders = vec![];
    let file = tokio::fs::File::open(filename).await?;
    let mut lines = tokio::io::BufReader::new(file).lines();
    while let Some(line) = lines.next_line().await? {
        orders.push(Order::from_str(&line)?);
    }

    Ok(orders)
}

#[cfg(test)]
mod test {
    use std::str::FromStr;

    use super::Order;

    #[test]
    fn order_from_correct_str() {
        let order = Order::from_str("3,2").expect("Order was not properly parsed");

        assert_eq!(order.get_product(), 3);
        assert_eq!(order.get_qty(), 2);
    }

    #[test]
    fn order_from_correct_str_with_spaces() {
        let order = Order::from_str("   2  ,  1  ").expect("Order was not properly parsed");

        assert_eq!(order.get_product(), 2);
        assert_eq!(order.get_qty(), 1);
    }

    #[test]
    fn order_negative_error() {
        let order1 = Order::from_str("-2,1");
        let order2 = Order::from_str("-2,-1");
        let order3 = Order::from_str("2,-1");

        assert!(order1.is_err());
        assert!(order2.is_err());
        assert!(order3.is_err());
    }

    #[test]
    fn order_not_number() {
        let order1 = Order::from_str("as,cuatro");

        assert!(order1.is_err());
    }
}
