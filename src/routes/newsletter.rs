use crate::{
    domain::SubscriberEmail,
    email_client::EmailClient,
    telemetry::{error_chain_fmt, spawn_blocking_with_tracing},
};
use ::actix_web::HttpResponse;
use actix_web::{http::header::HeaderMap, web, HttpRequest, ResponseError};
use anyhow::{anyhow, Context};
use argon2::{Argon2, PasswordHash, PasswordVerifier};
use reqwest::{
    header::{self, HeaderValue},
    StatusCode,
};
use secrecy::{ExposeSecret, Secret};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(serde::Deserialize)]
pub struct BodyData {
    title: String,
    content: Content,
}

#[derive(serde::Deserialize)]
pub struct Content {
    html: String,
    text: String,
}

struct ConfirmedSubscriber {
    email: SubscriberEmail,
}

#[tracing::instrument(name = "Publish a newsletter issue", skip(pool,body,email_client,request),fields(username=tracing::field::Empty,user_id=tracing::field::Empty))]
pub async fn publish_newsletter(
    body: web::Json<BodyData>,
    pool: web::Data<PgPool>,
    email_client: web::Data<EmailClient>,
    request: HttpRequest,
) -> Result<HttpResponse, PulishError> {
    let credentials = basic_authentication(request.headers()).map_err(PulishError::AuthError)?;
    tracing::Span::current().record("username", &tracing::field::display(&credentials.username));
    let user_id = validate_credentials(credentials, &pool).await?;
    tracing::Span::current().record("user_id", &tracing::field::display(&user_id));
    let subscribers = get_confirm_subscribers(&pool).await?;
    for subscriber in subscribers {
        match subscriber {
            Ok(subscriber) => {
                email_client
                    .send_email(
                        &subscriber.email,
                        &body.title,
                        &body.content.html,
                        &body.content.text,
                    )
                    .await
                    .with_context(|| {
                        format!("Failed to send newsletter issue to { }", subscriber.email)
                    })?;
            }
            Err(error) => {
                tracing::warn!(error.cause_chain = ?error,"Skipping a confirmed subscriber. \
                Their store contact details are invalid", )
            }
        }
    }
    Ok(HttpResponse::Ok().finish())
}

#[tracing::instrument(name = "Get confirmed subscribers", skip(pool))]
async fn get_confirm_subscribers(
    pool: &PgPool,
) -> Result<Vec<Result<ConfirmedSubscriber, anyhow::Error>>, anyhow::Error> {
    let rows = sqlx::query!(
        r#"
            SELECT email FROM subscriptions WHERE status = 'confirmed'
        "#,
    )
    .fetch_all(pool)
    .await?;
    let confirmed_subscribers = rows
        .into_iter()
        // .filter_map
        .map(|r| match SubscriberEmail::parse(r.email) {
            Ok(email) => Ok(ConfirmedSubscriber { email }),
            Err(error) => Err(anyhow!(error)),
        })
        .collect();
    Ok(confirmed_subscribers)
}

struct Credentials {
    username: String,
    password: Secret<String>,
}

fn basic_authentication(headers: &HeaderMap) -> Result<Credentials, anyhow::Error> {
    let header_value = headers
        .get("Authorization")
        .context("The 'Authorization' header was missing")?
        .to_str()
        .context("The 'Authorization' header was not a valid UTF8 string.")?;
    let base64encode_segment = header_value
        .strip_prefix("Basic ")
        .context("The authorization scheme was not 'Basic'. ")?;
    let decode_bytes = base64::decode_config(base64encode_segment, base64::STANDARD)
        .context("Failed to base64-decode 'Basic' credentials.")?;
    let decode_credentials = String::from_utf8(decode_bytes)
        .context("The decoded credential string is not valid UTF8.")?;

    let mut credentials = decode_credentials.splitn(2, ':');

    let username = credentials
        .next()
        .ok_or_else(|| anyhow!("A username must be provided in 'Basic' auth."))?
        .to_string();
    let password = credentials
        .next()
        .ok_or_else(|| anyhow!("A password must be provided in 'Basic' auth."))?
        .to_string();
    Ok(Credentials {
        username,
        password: Secret::new(password),
    })
}

async fn validate_credentials(
    credentials: Credentials,
    pool: &PgPool,
) -> Result<uuid::Uuid, PulishError> {
    let (user_id, expected_password_hash) = get_stored_credentials(&credentials.username, &pool)
        .await
        .map_err(PulishError::UnexpectedError)?
        .ok_or_else(|| PulishError::AuthError(anyhow!("Unknown username")))?;

    spawn_blocking_with_tracing(move || {
        verify_password_hash(expected_password_hash, credentials.password)
    })
    .await
    .context("Failed to spawn blocking task.")
    .map_err(PulishError::UnexpectedError)??;

    Ok(user_id)
}

#[tracing::instrument(name = "Get stored credentials", skip(pool, username))]
pub async fn get_stored_credentials(
    username: &str,
    pool: &PgPool,
) -> Result<Option<(Uuid, Secret<String>)>, anyhow::Error> {
    let row = sqlx::query!(
        r#"  
            SELECT user_id,password_hash FROM users WHERE username = $1  
        "#,
        username
    )
    .fetch_optional(pool)
    .await
    .context("Failed to perform a query to validate auth credentials.")?
    .map(|row| (row.user_id, Secret::new(row.password_hash)));
    Ok(row)
}

#[tracing::instrument(
    name = "Verify password hash",
    skip(expected_password_hash, password_candidate)
)]
fn verify_password_hash(
    expected_password_hash: Secret<String>,
    password_candidate: Secret<String>,
) -> Result<(), PulishError> {
    let expected_password_hash = PasswordHash::new(expected_password_hash.expose_secret())
        .context("Failed to parse hash in PHC string format.")
        .map_err(PulishError::UnexpectedError)?;
    Argon2::default()
        .verify_password(
            password_candidate.expose_secret().as_bytes(),
            &expected_password_hash,
        )
        .context("Invalid password.")
        .map_err(PulishError::AuthError)
}

#[derive(thiserror::Error)]
pub enum PulishError {
    #[error("Authentication failed.")]
    AuthError(#[source] anyhow::Error),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl ResponseError for PulishError {
    fn status_code(&self) -> reqwest::StatusCode {
        match self {
            PulishError::UnexpectedError(_) => reqwest::StatusCode::INTERNAL_SERVER_ERROR,
            PulishError::AuthError(_) => reqwest::StatusCode::UNAUTHORIZED,
        }
    }

    fn error_response(&self) -> HttpResponse<actix_web::body::BoxBody> {
        match self {
            PulishError::AuthError(_) => {
                let mut response = HttpResponse::new(StatusCode::UNAUTHORIZED);
                let header_value = HeaderValue::from_str(r#"Basic realm="publish""#).unwrap();
                response
                    .headers_mut()
                    .insert(header::WWW_AUTHENTICATE, header_value);
                response
            }
            PulishError::UnexpectedError(_) => HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR),
        }
    }
}

impl std::fmt::Debug for PulishError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}
