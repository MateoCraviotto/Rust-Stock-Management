use clap::Parser;
use rand::seq::SliceRandom;
use std::{str::FromStr, sync::Arc};
use tokio::{io::AsyncBufReadExt, net::TcpStream, sync::Mutex};
use tp::{
    common::order::Order,
    ecommerce::args::Args,
    ecommerce::{
        network::connection::{read_socket, write_socket},
        purchases::purchase_state::PurchaseState,
    },
    log_level,
};

// Missing retry policy when the store is down or does not have stock
#[actix_rt::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    log_level!(args.verbosity);
    let store_port = get_closest_store(args.ports).unwrap(); // remove unwrap
    println!(
        "Running E-Commerce which will connect to {}:{}",
        args.ip, store_port
    );
    let client = Arc::new(Mutex::new(
        TcpStream::connect(format!("{}:{}", args.ip, store_port)).await?,
    ));
    let orders = read_orders(args.file).await?;
    let mut tasks = vec![];
    for order in orders.iter() {
        let order_clone = order.clone(); // clone order to send to store
        let client_clone = client.clone();
        // Create tokio task for each order
        let t = tokio::spawn(async move {
            match manage_purchase(order_clone, client_clone).await {
                Ok(state) => {
                    println!("Purchase state: {:?}", state.to_string());
                    match state {
                        PurchaseState::Confirm => {
                            println!("Purchase confirmed");
                        }
                        _ => {
                            println!("Purchase cancelled. Retrying with next closest store.");
                            // ports.pop
                            // get next closest store
                        }
                    }
                }
                Err(e) => {
                    println!("Error while managing purchase: {}", e);
                }
            };
        });
        tasks.push(t);
        //tokio::time::sleep(Duration::from_secs(2)).await;
    }

    let _ = futures::future::join_all(tasks).await;

    Ok(())
}

async fn manage_purchase(
    order: Order,
    store: Arc<Mutex<TcpStream>>,
) -> anyhow::Result<PurchaseState> {
    println!("Sending order to store: {:?}", order.to_string());
    write_socket(store.clone(), &order.to_string()).await?;
    println!("Sent order {:?} to store.", order.to_string());

    let answer = match read_socket(store.clone()).await {
        Ok(answer) => {
            println!(
                "Store answered {} to order {:?}",
                answer,
                order.get_product()
            );
            answer
        }
        Err(e) => {
            println!("Error while reading answer from store: {}", e);
            PurchaseState::Cancel.to_string()
        }
    };

    let answer_state = match PurchaseState::from_str(&answer) {
        Ok(s) => s,
        Err(e) => {
            println!("Error while parsing answer from store: {}", e);
            PurchaseState::Cancel
        }
    };

    match answer_state {
        PurchaseState::Reserve => {
            println!("Stock reserved");
            println!("Confirming purchase");
            let confirmation = format!("{}{}", PurchaseState::Confirm.to_string(), "\n");
            write_socket(store.clone(), &confirmation).await?;
            let answer = match read_socket(store.clone()).await {
                Ok(answer) => {
                    println!(
                        "Store answered: {} to order {:?}",
                        answer,
                        order.to_string()
                    );
                    answer
                }
                Err(e) => {
                    println!("Error while reading answer from store: {}", e);
                    PurchaseState::Cancel.to_string()
                }
            };
            let confirmation_state = match PurchaseState::from_str(&answer) {
                Ok(s) => s,
                Err(e) => {
                    println!("Error while parsing answer from store: {}", e);
                    PurchaseState::Cancel
                }
            };

            match confirmation_state {
                PurchaseState::Confirm => {
                    println!("Purchase {:?} confirmed", order.to_string());
                    Ok(PurchaseState::Confirm)
                }
                _ => {
                    println!(
                        "Purchase was not confirmed. Cancelling purchase {:?}.",
                        order.to_string()
                    ); // Should look in next store
                    Ok(PurchaseState::Cancel)
                }
            }
        }
        _ => {
            println!("The order {:?} was cancelled", order.to_string()); // check this
            Ok(PurchaseState::Cancel)
        }
    }
}

async fn read_orders(filename: String) -> anyhow::Result<Vec<Order>> {
    let mut orders = vec![];
    let file = tokio::fs::File::open(filename).await?;
    let mut lines = tokio::io::BufReader::new(file).lines();
    while let Some(line) = lines.next_line().await? {
        orders.push(Order::from_str(&line)?);
    }

    Ok(orders)
}

fn get_closest_store(ports: Vec<u16>) -> Option<u16> {
    // Get a random store port
    let port = ports.choose(&mut rand::thread_rng()).map(|x| *x);
    port
}
