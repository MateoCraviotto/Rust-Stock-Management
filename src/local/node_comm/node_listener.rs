use std::net::Ipv4Addr;

use tokio::{net::TcpListener, task::JoinHandle, select};
use tokio_util::sync::CancellationToken;

use crate::{info, local::node_comm::node_communication::NodeCommunication};

struct NodeListener{
    cancel: CancellationToken,
    task_handle: JoinHandle<anyhow::Result<()>>
}

impl NodeListener{
    pub fn start(port: u16, ip: Ipv4Addr) -> Self{
        let cancel = CancellationToken::new();

        let handle = tokio::spawn(Self::run(port, ip, cancel.clone()));

        Self{
            cancel, 
            task_handle: handle
        }
    }

    async fn run(port: u16, ip: Ipv4Addr, cancel: CancellationToken) -> anyhow::Result<()>{
        //TODO: conectar a todos los demas que nos pasen
        let listener = TcpListener::bind(format!("{}:{}", &ip, port)).await?;

        let mut node_comms = vec![];

        'accept: loop{
            select!{
                conn_result = listener.accept() => {
                    let (stream, addr) = conn_result?;
                    info!(format!("New node connection for {}", addr));
                    node_comms.push(NodeCommunication::new(stream, cancel.clone()))
                }

                _ = cancel.cancelled() => {
                    info!("Got order to take down the node TCP stream");
                    break 'accept;
                }
            }
        }

        let _ = futures::future::join_all(node_comms.into_iter().map(|nc| {nc.shutdown()}));


        Ok(())
    }

    fn shutdown(&self) {
        self.cancel.cancel();
    }
}


