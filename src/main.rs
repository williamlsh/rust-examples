use actix_web::web::Data;
use actix_web::{web, App, HttpServer};
use mediasoup::worker_manager::WorkerManager;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();

    // We will reuse the same worker manager across all connections, this is more than enough for
    // this use case
    let worker_manager = Data::new(WorkerManager::new());
    HttpServer::new(move || {
        App::new()
            .app_data(worker_manager.clone())
            .route("/ws", web::get().to(echo::ws_index))
    })
    // 2 threads is plenty for this example, default is to have as many threads as CPU cores
    .workers(2)
    .bind("127.0.0.1:3000")?
    .run()
    .await
}
