use clap::Parser;
use rand::{seq::SliceRandom, Rng};
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
    error, log_level,
};

#[actix_rt::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let mut ports = args.ports;
    log_level!(args.verbosity);

    let mut orders = read_orders(args.file.clone()).await?;

    while !ports.is_empty() && !orders.is_empty() {
        let store_port = match get_closest_store(ports.clone()) {
            Some(port) => port,
            None => {
                println!("No stores available");
                return Ok(());
            }
        };
        // Remove port from args.ports
        let index = ports.clone().iter().position(|&x| x == store_port).unwrap();
        ports.remove(index);

        println!(
            "Running E-Commerce which will connect to {}:{}",
            args.ip, store_port
        );
        let connection = match TcpStream::connect(format!("{}:{}", args.ip, store_port)).await {
            Ok(c) => c,
            Err(e) => {
                println!("Error while connecting to store: {}", e);
                continue;
            }
        };
        let client = Arc::new(Mutex::new(connection));
        let mut tasks = vec![];
        for order in orders.into_iter() {
            let client_clone = client.clone();
            // Create tokio task for each order
            let t: tokio::task::JoinHandle<Result<(), Order>> = tokio::spawn(async move {
                match manage_purchase(order, client_clone).await {
                    Ok(state) => match state {
                        PurchaseState::Commit(id) => {
                            println!("Purchase completed. Id: {}", id);
                            return Ok(());
                        }
                        _ => {
                            println!("Purchase cancelled.");
                            return Ok(());
                        }
                    },
                    Err(e) => {
                        error!(format!(
                            "There was an error while managing a purchase {:?}",
                            e
                        ));
                        return Err(order);
                    }
                };
            });
            tasks.push(t);
        }

        orders = futures::future::join_all(tasks)
            .await
            .into_iter()
            .flat_map(|result| match result {
                Ok(Err(o)) => Some(o),
                _ => None,
            })
            .collect();
    }
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
            let msg = determine_commit_or_cancel(id);
            write_socket(store.clone(), &msg).await?;
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
    let port = ports.choose(&mut rand::thread_rng()).copied();
    port
}

fn determine_commit_or_cancel(id: u128) -> String {
    let mut rng = rand::thread_rng();
    let random_number: u8 = rng.gen_range(0..10);
    if random_number == 0 {
        println!("Cancelling purchase. Took to long for products to arrive.");
        PurchaseState::Cancel(id).to_string()
    } else {
        println!("Committing purchase");
        PurchaseState::Commit(id).to_string()
    }
}
