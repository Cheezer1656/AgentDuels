use actix_web::{App, HttpServer};

mod game_server;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    tokio::spawn(async move {
        if let Err(e) = game_server::GameServer::default().listen(("127.0.0.1", 8081)).await {
            eprintln!("Game server stopped with error: {}", e);
        };
    });

    HttpServer::new(move || {
        App::new()
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}