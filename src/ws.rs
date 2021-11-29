use crate::participant::ParticipantConnection;
use crate::room::RoomId;
use crate::rooms_registry::RoomsRegistry;
use actix_web::web::{Data, Payload, Query};
use actix_web::{web, App, Error, HttpRequest, HttpResponse, HttpServer};
use actix_web_actors::ws;
use mediasoup::prelude::*;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QueryParameters {
    room_id: Option<RoomId>,
}

async fn ws_index(
    query_parameters: Query<QueryParameters>,
    request: HttpRequest,
    worker_manager: Data<WorkerManager>,
    rooms_registry: Data<RoomsRegistry>,
    stream: Payload,
) -> Result<HttpResponse, Error> {
    let room = match query_parameters.room_id {
        Some(room_id) => {
            rooms_registry
                .get_or_create_room(&worker_manager, room_id)
                .await
        }
        None => rooms_registry.create_room(&worker_manager).await,
    };

    let room = match room {
        Ok(room) => room,
        Err(error) => {
            eprintln!("{}", error);

            return Ok(HttpResponse::InternalServerError().finish());
        }
    };

    match ParticipantConnection::new(room).await {
        Ok(server) => ws::start(server, &request, stream),
        Err(error) => {
            eprintln!("{}", error);

            Ok(HttpResponse::InternalServerError().finish())
        }
    }
}

pub async fn run() -> std::io::Result<()> {
    // We will reuse the same worker manager across all connections, this is more than enough for
    // this use case.
    let worker_manager = Data::new(WorkerManager::new());
    // Rooms registry will hold all the active rooms.
    let rooms_registry = Data::new(RoomsRegistry::default());

    HttpServer::new(move || {
        App::new()
            .app_data(worker_manager.clone())
            .app_data(rooms_registry.clone())
            .route("/ws", web::get().to(ws_index))
    })
    .workers(2)
    .bind("127.0.0.1:3000")?
    .run()
    .await
}
