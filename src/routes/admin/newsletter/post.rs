use crate::{
    authentication::UserId,
    idempotency::{saved_response, try_processing, IdempotencyKey},
    utils::{e400, e500, see_other},
};
use ::actix_web::HttpResponse;
use actix_web::web;
use actix_web_flash_messages::FlashMessage;
use anyhow::Context;

use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

#[derive(serde::Deserialize)]
pub struct FormData {
    title: String,
    text_content: String,
    html_content: String,
    idempotency_key: String,
}

#[tracing::instrument(name = "Publish a newsletter issue", skip(pool,form),fields(username=tracing::field::Empty,user_id=tracing::field::Empty))]
pub async fn publish_newsletter(
    form: web::Form<FormData>,
    pool: web::Data<PgPool>,
    user_id: web::ReqData<UserId>,
) -> Result<HttpResponse, actix_web::Error> {
    let user_id = user_id.into_inner();
    let FormData {
        title,
        text_content,
        html_content,
        idempotency_key,
    } = form.0;
    let idempotency_key: IdempotencyKey = idempotency_key.try_into().map_err(e400)?;
    let mut transaction = match try_processing(&pool, &idempotency_key, *user_id)
        .await
        .map_err(e500)?
    {
        crate::idempotency::NextAction::StartProcessing(t) => t,
        crate::idempotency::NextAction::ReturnSaveResponse(http_response) => {
            FlashMessage::info("The newsletter issue has been published!").send();
            return Ok(http_response);
        }
    };
    let issue_id = insert_newsletter_issue(&mut transaction, &title, &text_content, &html_content)
        .await
        .context("Failed to store newsletter issue details")
        .map_err(e500)?;
    enqueue_delivery_tasks(&mut transaction, issue_id)
        .await
        .context("Failed to enqueue delivery tasks")
        .map_err(e500)?;
    // let subscribers = get_confirm_subscribers(&pool).await.map_err(e500)?;
    // for subscriber in subscribers {
    //     match subscriber {
    //         Ok(subscriber) => {
    //             email_client
    //                 .send_email(&subscriber.email, &title, &html_content, &text_content)
    //                 .await
    //                 .with_context(|| {
    //                     format!("Failed to send newsletter issue to { }", subscriber.email)
    //                 })
    //                 .map_err(e500)?;
    //         }
    //         Err(error) => {
    //             tracing::warn!(error.cause_chain = ?error,"Skipping a confirmed subscriber. \
    //             Their store contact details are invalid", )
    //         }
    //     }
    // }
    FlashMessage::info("The newsletter issue has been published!").send();
    let response = see_other("/admin/newsletters");
    let response = saved_response(transaction, &idempotency_key, *user_id, response)
        .await
        .map_err(e500)?;
    Ok(response)
}

// #[tracing::instrument(name = "Get confirmed subscribers", skip(pool))]
// async fn get_confirm_subscribers(
//     pool: &PgPool,
// ) -> Result<Vec<Result<ConfirmedSubscriber, anyhow::Error>>, anyhow::Error> {
//     let rows = sqlx::query!(
//         r#"
//             SELECT email FROM subscriptions WHERE status = 'confirmed'
//         "#,
//     )
//     .fetch_all(pool)
//     .await?;
//     let confirmed_subscribers = rows
//         .into_iter()
//         // .filter_map
//         .map(|r| match SubscriberEmail::parse(r.email) {
//             Ok(email) => Ok(ConfirmedSubscriber { email }),
//             Err(error) => Err(anyhow!(error)),
//         })
//         .collect();
//     Ok(confirmed_subscribers)
// }

#[tracing::instrument(skip_all)]
async fn insert_newsletter_issue(
    transaction: &mut Transaction<'_, Postgres>,
    title: &str,
    text_content: &str,
    html_content: &str,
) -> Result<Uuid, sqlx::Error> {
    let newsletter_issue_id = Uuid::new_v4();
    sqlx::query!(
        r#"
           INSERT INTO newsletter_issues (
                newsletter_issues_id,
                title,
                text_content,
                html_content,
                published_at
        )
        VALUES ($1,$2,$3,$4,now())
        "#,
        newsletter_issue_id,
        title,
        text_content,
        html_content
    )
    .execute(transaction)
    .await?;

    Ok(newsletter_issue_id)
}

#[tracing::instrument(skip_all)]
async fn enqueue_delivery_tasks(
    transaction: &mut Transaction<'_, Postgres>,
    nesletter_issue_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
           INSERT INTO issues_delivery_queue (
                newsletter_issues_id,
               subscriber_email
        )
        SELECT $1 ,email 
        FROM subscriptions
        WHERE status = 'confirmed'
        "#,
        nesletter_issue_id
    )
    .execute(transaction)
    .await?;

    Ok(())
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
