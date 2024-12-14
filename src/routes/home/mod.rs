use ::actix_web::{HttpRequest, HttpResponse};
use actix_web::http::header::ContentType;

pub async fn home(_req: HttpRequest) -> HttpResponse {
    HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(include_str!("home.html"))
}
