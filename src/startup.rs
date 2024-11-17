use crate::routes::{health_check, subscriptions};
use ::actix_web::{web, App, HttpServer};
use actix_web::dev::Server;
use sqlx::PgPool;
use std::net::TcpListener;
use tracing_actix_web::TracingLogger;

pub fn run(listener: TcpListener, db_poll: PgPool) -> Result<Server, std::io::Error> {
    let connection = web::Data::new(db_poll);
    let server: actix_web::dev::Server = HttpServer::new(move || {
        App::new()
            .wrap(TracingLogger::default())
            .route("/health_check", web::get().to(health_check))
            .route("/subscriptions", web::post().to(subscriptions))
            .app_data(connection.clone())
    })
    .listen(listener)?
    .run();
    Ok(server)
}
