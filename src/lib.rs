use std::net::TcpListener;
use ::actix_web::{web, App, HttpRequest, HttpResponse, HttpServer, Responder};
use actix_web::dev::Server;

async fn health_check(_req: HttpRequest) -> impl Responder {
    HttpResponse::Ok()
}

pub fn run(listener: TcpListener) -> Result<Server, std::io::Error> {
    let server: actix_web::dev::Server =
        HttpServer::new(|| App::new().route("/health_check", web::get().to(health_check)))
            .listen(listener)?
            .run();
    Ok(server)
}
