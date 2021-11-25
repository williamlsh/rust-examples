use std::sync::Arc;

use mediasoup::worker_manager::WorkerManager;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    let addr = "127.0.0.1:9002";
    let listener = TcpListener::bind(&addr).await.expect("Can't listen");
    println!("Listening on: {}", addr);

    let worker_manager = Arc::new(WorkerManager::new());

    while let Ok((stream, _)) = listener.accept().await {
        let peer = stream
            .peer_addr()
            .expect("connected streams should have a peer address");
        println!("Peer address: {}", peer);

        tokio::spawn(echo::handle_connection(
            peer,
            stream,
            worker_manager.clone(),
        ));
    }
}
