use std::{net::SocketAddr, sync::Arc};

use axum::{
    error_handling::HandleErrorExt,
    http::StatusCode,
    routing::{get, service_method_routing as service},
    AddExtensionLayer, Router,
};
use echo::websocket_handler;
use mediasoup::worker_manager::WorkerManager;
use tower_http::services::ServeDir;

#[tokio::main]
async fn main() {
    // We will reuse the same worker manager across all connections, this is more than enough for
    // this use case.
    let worker_manager = Arc::new(WorkerManager::new());

    let app = Router::new()
        .fallback(
            service::get(ServeDir::new("assets").append_index_html_on_directories(true))
                .handle_error(|error: std::io::Error| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("Unhandled internal error: {}", error),
                    )
                }),
        )
        .route("/ws", get(websocket_handler))
        .layer(AddExtensionLayer::new(worker_manager.clone()));

    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}
