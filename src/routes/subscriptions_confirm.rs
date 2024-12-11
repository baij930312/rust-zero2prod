use ::actix_web::HttpResponse;
use actix_web::web;
use actix_web::ResponseError;
use anyhow::Context;
use sqlx::PgPool;
use uuid::Uuid;

use crate::telemetry::error_chain_fmt;

#[derive(serde::Deserialize)]
pub struct Parameters {
    subscription_token: String,
}

#[tracing::instrument(name = "Confirm a pending subscriber", skip(parameters, pool))]
pub async fn confirm(
    parameters: web::Query<Parameters>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, ConfirmError> {
    let id = get_subscriber_id_from_token(&pool, &parameters.subscription_token)
        .await
        .context("查询订阅id失败")?;

    match id {
        Some(subscriber_id) => {
            confirm_subscriber(&pool, subscriber_id)
                .await
                .context("更新确认状态失败")?;
        }
        None => return Err(ConfirmError::UnauthorizedTokenError("没有找到id".into())),
    }
    Ok(HttpResponse::Ok().finish())
}

#[tracing::instrument(name = "Get subscriber_id from token", skip(token, pool))]
pub async fn get_subscriber_id_from_token(
    pool: &PgPool,
    token: &str,
) -> Result<Option<Uuid>, sqlx::Error> {
    let result = sqlx::query!(
        r#"
           SELECT subscriber_id FROM subscription_tokens WHERE subscription_token = $1
        "#,
        token
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to execute query:{:?}", e);
        e
    })?;
    Ok(result.map(|r| r.subscriber_id))
}

#[tracing::instrument(name = "Mark subscriber as confirmed", skip(subscriber_id, pool))]
pub async fn confirm_subscriber(pool: &PgPool, subscriber_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
           UPDATE subscriptions SET status = 'confirmed' WHERE id = $1
        "#,
        subscriber_id
    )
    .execute(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to execute query:{:?}", e);
        e
    })?;

    Ok(())
}

#[derive(thiserror::Error)]
pub enum ConfirmError {
    #[error("{0}")]
    UnauthorizedTokenError(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl ResponseError for ConfirmError {
    fn status_code(&self) -> reqwest::StatusCode {
        match self {
            ConfirmError::UnauthorizedTokenError(_) => reqwest::StatusCode::UNAUTHORIZED,
            ConfirmError::UnexpectedError(_) => reqwest::StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl std::fmt::Debug for ConfirmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}
 