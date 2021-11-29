use videoroom::ws::run;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();

    run().await
}
