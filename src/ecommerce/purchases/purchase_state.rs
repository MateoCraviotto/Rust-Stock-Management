use std::str::FromStr;

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
struct PurchaseNumber(u32);


#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub enum PurchaseState {
    Reserve,
    Confirm,
    Cancel,
}

impl ToString for PurchaseState{
    fn to_string(&self) -> String {
        match self {
            PurchaseState::Reserve => "Reserve".to_string(),
            PurchaseState::Confirm => "Confirm".to_string(),
            PurchaseState::Cancel => "Cancel".to_string(),
        }
    }
}

impl FromStr for PurchaseState{
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim() {
            "Reserve" => Ok(PurchaseState::Reserve),
            "Confirm" => Ok(PurchaseState::Confirm),
            "Cancel" => Ok(PurchaseState::Cancel),
            _ => Err(anyhow::anyhow!("The given string is not a valid PurchaseState")),
        }
    }
}
/*
struct PurchaseCoordinator {
    log: HashMap<PurchaseNumber, PurchaseState>,
    //socket: UdpSocket,
    responses: Arc<(Mutex<HashMap<u16, PurchaseState>>, Condvar)>,
}

impl PurchaseCoordinator {
    async fn new() -> Self {
        let mut ret = PurchaseCoordinator {
            log: HashMap::new(),
            //socket: UdpSocket::bind(TRANSACTION_COORDINATOR_ADDR).unwrap(),
            responses: Arc::new((Mutex::new(HashMap::new()), Condvar::new())),
        };

        let mut clone: PurchaseCoordinator = ret.clone();
        // spawn tokio task with the purchase_receiver
        tokio::spawn(async move {
            clone.listen_purchases().await;
        });

        ret
    }

    fn clone(&self) -> Self {
        PurchaseCoordinator {
            log: self.log.clone(),
            //socket: self.socket.try_clone().unwrap(),
            responses: self.responses.clone(),
        }
    }

    async fn listen_purchases(net: Addr<Listener>) -> anyhow::Result<()> {
        println!("Reading commands from STDIN");
    
        let mut lines = BufReader::new(tokio::io::stdin()).lines();
    
        // Save all the times that we tell the network to go up so we wait on all of them.
        // All of them should finish at the same time because of the cancellation tokens
        let mut t = vec![];
        t.push(start_listener(net.clone()));
    
        'main: loop {
            let r = lines.next_line().await;
            if let Ok(Some(line)) = r{
                match Command::from_str(&line){
                    Ok(c) => match c {
                        Command::Shutdown => {
                            info!("Shutdown order was given");
                            net.do_send(ListenerState::Shutdown);
                            break 'main;
                        },
                        Command::NetUp => {
                            info!("Network Up order was given");
                            t.push(start_listener(net.clone()));
                            println!("Network was connected");
                        },
                        Command::NetDown => {
                            info!("Network Down order was given");
                            let _ = net.do_send(ListenerState::Down);
                            // Await all current messages to have a 'clean' environment
                            // in next iteration. Will block until all disconnections happened
                            let _ = futures::future::join_all(t).await;
                            t = vec![];
                            println!("Network was disconnected");
                        },
                        Command::Sell(o) => {
                            info!(format!("New order was issued: {:?}", o));
                            //TODO: get the address of the actor that modifies things
                            //TODO: See how it modifies
                        },
                        Command::SellFromFile(f) => {
                            info!(format!("New orders were issued. Info in: {:?}", f));
                            //TODO: read file, read line by line, send to actor that modifies things
                        },
                    },
                    Err(e) => {
                        error!(format!("The given command was not able to parse: {:?} ", e));
                        print_commands();
                    },
                }
            }
        }
    
        let _ = futures::future::join_all(t).await;
        Ok(())
    }

}
*/