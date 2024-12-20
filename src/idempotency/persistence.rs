use actix_web::{body::to_bytes, HttpResponse};

use anyhow::anyhow;
use reqwest::StatusCode;
use sqlx::{postgres::PgHasArrayType, PgPool, Postgres, Transaction};
use uuid::Uuid;

use super::IdempotencyKey;

#[derive(Debug, sqlx::Type)]
#[sqlx(type_name = "header_pair")]
struct HeaderPairRecord {
    name: String,
    value: Vec<u8>,
}

impl PgHasArrayType for HeaderPairRecord {
    fn array_type_info() -> sqlx::postgres::PgTypeInfo {
        sqlx::postgres::PgTypeInfo::with_name("_header_pair")
    }
}

pub async fn get_saved_response(
    pool: &PgPool,
    idempotency_key: &IdempotencyKey,
    user_id: Uuid,
) -> Result<Option<HttpResponse>, anyhow::Error> {
    let save_response = sqlx::query!(
        r#"
            SELECT 
                response_status_code as "response_status_code!",
                response_headers as "response_headers!: Vec<HeaderPairRecord>",
                response_body as "response_body!"
            FROM idempotency
            WHERE
                user_id = $1 AND
                idempotency_key = $2
        "#,
        user_id,
        idempotency_key.as_ref()
    )
    .fetch_optional(pool)
    .await?;
    if let Some(r) = save_response {
        let status_code = StatusCode::from_u16(r.response_status_code.try_into()?)?;
        let mut response = HttpResponse::build(status_code);

        for HeaderPairRecord { name, value } in r.response_headers {
            response.append_header((name, value));
        }
        Ok(Some(response.body(r.response_body)))
    } else {
        Ok(None)
    }
}

pub async fn saved_response(
    mut transaction: Transaction<'_, Postgres>,
    idempotency_key: &IdempotencyKey,
    user_id: Uuid,
    http_response: HttpResponse,
) -> Result<HttpResponse, anyhow::Error> {
    let (response_head, body) = http_response.into_parts();

    let body = to_bytes(body).await.map_err(|e| anyhow!("{}", e))?;
    let status_code = response_head.status().as_u16() as i16;
    let headers = {
        let mut h = Vec::with_capacity(response_head.headers().len());
        for (name, value) in response_head.headers().iter() {
            let name = name.as_str().to_owned();
            let value = value.as_bytes().to_owned();
            h.push(HeaderPairRecord { name, value });
        }
        h
    };

    sqlx::query_unchecked!(
        r#"
        UPDATE idempotency SET 
            response_status_code = $3,
            response_headers = $4,
            response_body = $5
        WHERE
             user_id = $1  AND  
             idempotency_key = $2
    "#,
        user_id,
        idempotency_key.as_ref(),
        status_code,
        headers,
        body.as_ref()
    )
    .execute(&mut transaction)
    .await?;
    transaction.commit().await?;
    let http_response = response_head.set_body(body).map_into_boxed_body();
    Ok(http_response)
}

#[allow(clippy::large_enum_variant)]
pub enum NextAction {
    StartProcessing(Transaction<'static, Postgres>),
    ReturnSaveResponse(HttpResponse),
}

pub async fn try_processing(
    pool: &PgPool,
    idempotency_key: &IdempotencyKey,
    user_id: Uuid,
) -> Result<NextAction, anyhow::Error> {
    let mut transaction = pool.begin().await?;
    let n_inserted_rows = sqlx::query!(
        r#"
        INSERT INTO idempotency (
            user_id,
            idempotency_key,
            created_at
        )
        VALUES ($1,$2,now())
        ON CONFLICT DO NOTHING
   "#,
        user_id,
        idempotency_key.as_ref()
    )
    .execute(&mut transaction)
    .await?
    .rows_affected();
    if n_inserted_rows > 0 {
        Ok(NextAction::StartProcessing(transaction))
    } else {
        let saved_response = get_saved_response(pool, idempotency_key, user_id)
            .await?
            .ok_or_else(|| anyhow!("We expected a saved response, we didn't find it"))?;
        Ok(NextAction::ReturnSaveResponse(saved_response))
    }
}

#[tracing::instrument(skip_all)]
async fn delete_idempotency(
    pool: &PgPool,
    idempotency_key: &IdempotencyKey,
    user_id: Uuid,
) -> Result<(), anyhow::Error> {
    sqlx::query!(
        r#"
        DELETE FROM idempotency
        WHERE
            user_id = $1 AND idempotency_key = $2
        "#,
        user_id,
        idempotency_key.as_ref()
    )
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn delete_expire_idempotencys(pool: &PgPool) -> Result<(), anyhow::Error> {
    let result = sqlx::query!(
        r#"
        DELETE FROM idempotency
        WHERE
           created_at < (NOW() - INTERVAL '5 minute')
        "#,
    )
    .execute(pool)
    .await;
    if let Err(e) = result {
        tracing::error!(
            error.cause = ?e,
            error.message = %e,
            "Failed to delete expire idempotencys.",
        )
    }
    Ok(())
}

pub async fn delete_all_idempotencys(pool: &PgPool) -> Result<(), anyhow::Error> {
    let result = sqlx::query!(
        r#"
        TRUNCATE TABLE idempotency
        "#,
    )
    .execute(pool)
    .await;
    if let Err(e) = result {
        tracing::error!(
            error.cause = ?e,
            error.message = %e,
            "Failed to delete all idempotencys.",
        )
    }
    Ok(())
}
