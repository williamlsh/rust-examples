use actix_web::{get, web, App, HttpServer};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new()
            .data(AppState {
                app_name: String::from("hello"),
            })
            .service(index)
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}

struct AppState {
    app_name: String,
}

#[get("/")]
async fn index(data: web::Data<AppState>) -> String {
    let app_name = &data.app_name;

    format!("Hello {}!", app_name)
}
