use ::actix_web::HttpResponse;
use actix_web::{cookie::Cookie, error::InternalError, web};

use actix_web_flash_messages::FlashMessage;
use reqwest::header::LOCATION;
use secrecy::Secret;
use sqlx::PgPool;

use crate::{
    authentication::{validate_credentials, Credentials},
    telemetry::error_chain_fmt,
};

#[derive(serde::Deserialize)]
pub struct FormData {
    username: String,
    password: Secret<String>,
}

#[tracing::instrument(name = "login", skip(pool,from),fields(username=tracing::field::Empty,user_id=tracing::field::Empty))]
pub async fn login(
    from: web::Form<FormData>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, InternalError<LoginError>> {
    let credentials = Credentials {
        username: from.0.username,
        password: from.0.password,
    };
    tracing::Span::current().record("username", &tracing::field::display(&credentials.username));
    match validate_credentials(credentials, &pool).await {
        Ok(user_id) => {
            tracing::Span::current().record("user_id", &tracing::field::display(&user_id));
            Ok(HttpResponse::SeeOther()
                .insert_header((LOCATION, "/"))
                .finish())
        }
        Err(e) => {
            let e = match e {
                crate::authentication::AuthError::InvalidCredentials(e) => {
                    LoginError::AuthError(e.into())
                }
                crate::authentication::AuthError::UnexpectedError(e) => {
                    LoginError::UnexpectedError(e.into())
                }
            };
            FlashMessage::error(e.to_string()).send();
            let response = HttpResponse::SeeOther()
                .insert_header((LOCATION, format!("/login")))
         
                .finish();
            Err(InternalError::from_response(e, response))
        }
    }
}

#[derive(thiserror::Error)]
pub enum LoginError {
    #[error("Authentication failed.")]
    AuthError(#[source] anyhow::Error),
    #[error("Something went wrong")]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for LoginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}
