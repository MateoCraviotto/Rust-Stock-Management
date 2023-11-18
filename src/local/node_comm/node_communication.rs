use tokio::{task::JoinHandle, net::TcpStream};
use tokio_util::sync::CancellationToken;

pub struct NodeCommunication {
    task_handle: JoinHandle<anyhow::Result<()>>
}

impl NodeCommunication {
    pub fn new(stream: TcpStream, cancel: CancellationToken) -> Self{
        Self { task_handle: tokio::spawn(Self::run(stream, cancel))}
    }

    async fn run(stream: TcpStream, cancel: CancellationToken) -> anyhow::Result<()>{
        // Cuando me manda Hello, tengo que decirle al actor de la tienda que esa nueva tienda existe (o volvio a estar online) y que le diga todos los stock


        // Tengo que enviar cierta informacion a algun lado.
        //      Como consigo esa info para enviar
        //      A quien le devuelvo esa info
        // Tengo que recibir la info y mandarla a la tienda
        // Tengo que escuchar por cierres
        todo!()
    }

    pub async fn shutdown(self) -> anyhow::Result<()>{
        self.task_handle.await?
    }
}