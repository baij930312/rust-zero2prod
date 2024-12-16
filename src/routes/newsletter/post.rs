use crate::{domain::SubscriberEmail, email_client::EmailClient, utils::{e500, see_other}};
use ::actix_web::HttpResponse;
use actix_web::web;
use actix_web_flash_messages::FlashMessage;
use anyhow::{anyhow, Context};

use sqlx::PgPool;

#[derive(serde::Deserialize)]
pub struct FormData {
    title: String,
    text_content: String,
    html_content: String,
}

struct ConfirmedSubscriber {
    email: SubscriberEmail,
}

#[tracing::instrument(name = "Publish a newsletter issue", skip(pool,from,email_client),fields(username=tracing::field::Empty,user_id=tracing::field::Empty))]
pub async fn publish_newsletter(
    from: web::Form<FormData>,
    pool: web::Data<PgPool>,
    email_client: web::Data<EmailClient>,
) -> Result<HttpResponse, actix_web::Error> {
    // let credentials = basic_authentication(request.headers()).map_err(PulishError::AuthError)?;
    // tracing::Span::current().record("username", &tracing::field::display(&credentials.username));
    // let user_id = validate_credentials(credentials, &pool)
    //     .await
    //     .map_err(|e| match e {
    //         AuthError::InvalidCredentials(error) => PulishError::AuthError(error),
    //         AuthError::UnexpectedError(error) => PulishError::UnexpectedError(error),
    //     })?;
    // tracing::Span::current().record("user_id", &tracing::field::display(&user_id));
    let subscribers = get_confirm_subscribers(&pool).await.map_err(e500)?;
    for subscriber in subscribers {
        match subscriber {
            Ok(subscriber) => {
                email_client
                    .send_email(
                        &subscriber.email,
                        &from.0.title,
                        &from.0.html_content,
                        &from.0.text_content,
                    )
                    .await
                    .with_context(|| {
                        format!("Failed to send newsletter issue to { }", subscriber.email)
                    })
                    .map_err(e500)?;
            }
            Err(error) => {
                tracing::warn!(error.cause_chain = ?error,"Skipping a confirmed subscriber. \
                Their store contact details are invalid", )
            }
        }
    }
    FlashMessage::info("The newsletter issue has been published!").send();
    Ok(see_other("/admin/newsletters"))
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

// fn basic_authentication(headers: &HeaderMap) -> Result<Credentials, anyhow::Error> {
//     let header_value = headers
//         .get("Authorization")
//         .context("The 'Authorization' header was missing")?
//         .to_str()
//         .context("The 'Authorization' header was not a valid UTF8 string.")?;
//     let base64encode_segment = header_value
//         .strip_prefix("Basic ")
//         .context("The authorization scheme was not 'Basic'. ")?;
//     let decode_bytes = base64::decode_config(base64encode_segment, base64::STANDARD)
//         .context("Failed to base64-decode 'Basic' credentials.")?;
//     let decode_credentials = String::from_utf8(decode_bytes)
//         .context("The decoded credential string is not valid UTF8.")?;

//     let mut credentials = decode_credentials.splitn(2, ':');

//     let username = credentials
//         .next()
//         .ok_or_else(|| anyhow!("A username must be provided in 'Basic' auth."))?
//         .to_string();
//     let password = credentials
//         .next()
//         .ok_or_else(|| anyhow!("A password must be provided in 'Basic' auth."))?
//         .to_string();
//     Ok(Credentials {
//         username,
//         password: Secret::new(password),
//     })
// }

// #[derive(thiserror::Error)]
// pub enum PulishError {
//     #[error("Authentication failed.")]
//     AuthError(#[source] anyhow::Error),
//     #[error(transparent)]
//     UnexpectedError(#[from] anyhow::Error),
// }

// impl ResponseError for PulishError {
//     fn status_code(&self) -> reqwest::StatusCode {
//         match self {
//             PulishError::UnexpectedError(_) => reqwest::StatusCode::INTERNAL_SERVER_ERROR,
//             PulishError::AuthError(_) => reqwest::StatusCode::UNAUTHORIZED,
//         }
//     }

//     fn error_response(&self) -> HttpResponse<actix_web::body::BoxBody> {
//         match self {
//             PulishError::AuthError(_) => {
//                 let mut response = HttpResponse::new(StatusCode::UNAUTHORIZED);
//                 let header_value = HeaderValue::from_str(r#"Basic realm="publish""#).unwrap();
//                 response
//                     .headers_mut()
//                     .insert(header::WWW_AUTHENTICATE, header_value);
//                 response
//             }
//             PulishError::UnexpectedError(_) => HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR),
//         }
//     }
// }

// impl std::fmt::Debug for PulishError {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         error_chain_fmt(self, f)
//     }
// }
