use clap::Parser;
use rand::seq::SliceRandom;
use std::{str::FromStr, sync::Arc};
use tokio::{net::TcpStream, sync::Mutex};
use tp::{
    common::order::read_orders,
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
        let t: tokio::task::JoinHandle<()> = tokio::spawn(async move {
            match manage_purchase(order_clone, client_clone).await {
                Ok(state) => {
                    println!("Purchase state: {:?}", state.to_string());
                    match state {
                        PurchaseState::Commit(id) => {
                            println!("Purchase completed. Id: {}", id);
                        }
                        _ => {
                            println!("Purchase cancelled.");
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

    let answer = read_socket(store.clone()).await?;
    println!("Store answered {} to order {:?}", answer, order);

    let answer_state = PurchaseState::from_str(&answer)?;

    match answer_state {
        PurchaseState::Confirm(id) => {
            println!("Stock reserved");
            println!("Committing purchase");
            // todo: Add logic to cancel some purchases
            let commit_msg = PurchaseState::Commit(id).to_string();
            write_socket(store.clone(), &commit_msg).await?;
            let answer = read_socket(store.clone()).await?;
            let confirmation_state = PurchaseState::from_str(&answer)?;

            match confirmation_state {
                PurchaseState::Commit(id) => {
                    println!(
                        "Purchase with id {} and order {:?} confirmed",
                        id,
                        order.to_string()
                    );
                    Ok(confirmation_state)
                }
                _ => {
                    println!(
                        "Purchase was not confirmed. Cancelling purchase {:?}.",
                        order.to_string()
                    ); // Should look in next store
                    Ok(PurchaseState::Cancel(id))
                }
            }
        }
        PurchaseState::Cancel(id) => {
            println!(
                "The order {:?} with id {} was cancelled",
                order.to_string(),
                id
            ); // check this
            Ok(PurchaseState::Cancel(id))
        }
        PurchaseState::Commit(id) => {
            println!(
                "The order {:?} with id {} has been committed. Purchase was completed successfully",
                order.to_string(),
                id
            ); // check this
            Ok(PurchaseState::Commit(id))
        }
    }
}

fn get_closest_store(ports: Vec<u16>) -> Option<u16> {
    // Get a random store port
    let port = ports.choose(&mut rand::thread_rng()).map(|x| *x);
    port
}
