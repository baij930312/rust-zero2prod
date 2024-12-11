use ::actix_web::{web, HttpResponse};
use actix_web::ResponseError;
use anyhow::Context;
use chrono::Utc;
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::{
    domain::{NewSubscriber, SubscriberEmail, SubscriberName},
    email_client::EmailClient,
    startup::ApplicationBaseUrl, telemetry::error_chain_fmt,
};

#[derive(serde::Deserialize)]
pub struct FormData {
    email: String,
    name: String,
}

#[tracing::instrument(
    name = "Adding as a new  subscriber",
    skip(form,pool,email_client,base_url),
    fields(
        email= %form.email,
        name= %form.name,
    )
)]
pub async fn subscribe(
    form: web::Form<FormData>,
    pool: web::Data<PgPool>,
    email_client: web::Data<EmailClient>,
    base_url: web::Data<ApplicationBaseUrl>,
) -> Result<HttpResponse, SubscriberError> {
    let new_subscriber: NewSubscriber = form
        .0
        .try_into()
        .map_err(SubscriberError::ValidationTokenError)?;
    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection form the pool")?;
    let subscriber_id = insert_subscriber(&new_subscriber, &mut transaction)
        .await
        .context("Failed to insert new subscriber in the db")?;
    let subscription_token = generate_subscription_token();
    store_token(&mut transaction, subscriber_id, &subscription_token)
        .await
        .context("Failed to store the confirmation token for a new subscriber")?;
    send_confirmation_email(
        new_subscriber,
        &email_client,
        &base_url.0,
        &subscription_token,
    )
    .await
    .context("Failed to send a confirmation email")?;
    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to store a new subscriber")?;
    Ok(HttpResponse::Ok().finish())
}

#[tracing::instrument(
    name = "Send a confirmation email to a new subcriber",
    skip(new_subscriber, email_client, base_url)
)]
pub async fn send_confirmation_email(
    new_subscriber: NewSubscriber,
    email_client: &EmailClient,
    base_url: &str,
    subscription_token: &str,
) -> Result<(), reqwest::Error> {
    let confrimation_link = format!(
        "{}/subscriptions/confirm?subscription_token={}",
        base_url, subscription_token
    );
    let html_body = format!("Welcome to our newsletter! <br /> Click <a href=\"{}\">here</a> to confirm your subscription.",confrimation_link);
    let test_body = format!(
        "Welcome to our newsletter! \n  Visit {} to confirm your subscription.",
        confrimation_link
    );
    email_client
        .send_email(&new_subscriber.email, "Welcome", &html_body, &test_body)
        .await
}

#[tracing::instrument(
    name = "Saving as a new  subscriber",
    skip(new_subscriber, transaction)
)]
pub async fn insert_subscriber(
    new_subscriber: &NewSubscriber,
    transaction: &mut Transaction<'_, Postgres>,
) -> Result<Uuid, sqlx::Error> {
    let subscriber_id = Uuid::new_v4();
    sqlx::query!(
        r#"
            INSERT INTO subscriptions (id, email ,name , subscribed_at,status) VALUES ($1,$2,$3,$4,'pending_confirmation')
        "#,
        subscriber_id,
        new_subscriber.email.as_ref(),
        new_subscriber.name.as_ref(),
        Utc::now(),
    )
    .execute(transaction)
    .await
    .map_err(|e|{
        tracing::error!("Faild to execute query : {:?}",e);
        e
    })?;
    Ok(subscriber_id)
}

#[tracing::instrument(
    name = "Store subscription token in the db",
    skip(subscription_token, transaction)
)]
pub async fn store_token(
    transaction: &mut Transaction<'_, Postgres>,
    subscriber_id: Uuid,
    subscription_token: &str,
) -> Result<(), StoreTokenError> {
    sqlx::query!(
        r#"
            INSERT INTO subscription_tokens (subscription_token,subscriber_id) VALUES ($1,$2 )
        "#,
        subscription_token,
        subscriber_id,
    )
    .execute(transaction)
    .await
    .map_err(|e| {
        tracing::error!("Faild to execute query : {:?}", e);
        StoreTokenError(e)
    })?;
    Ok(())
}

fn generate_subscription_token() -> String {
    let mut rng = thread_rng();
    std::iter::repeat_with(|| rng.sample(Alphanumeric))
        .map(char::from)
        .take(25)
        .collect()
}

impl TryFrom<FormData> for NewSubscriber {
    type Error = String;

    fn try_from(value: FormData) -> Result<Self, Self::Error> {
        let name = SubscriberName::parse(value.name)?;
        let email = SubscriberEmail::parse(value.email)?;
        Ok(Self { email, name })
    }
}

#[derive(thiserror::Error)]
pub enum SubscriberError {
    #[error("{0}")]
    ValidationTokenError(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl ResponseError for SubscriberError {
    fn status_code(&self) -> reqwest::StatusCode {
        match self {
            SubscriberError::ValidationTokenError(_) => reqwest::StatusCode::BAD_REQUEST,
            SubscriberError::UnexpectedError(_) => reqwest::StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl std::fmt::Debug for SubscriberError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

pub struct StoreTokenError(sqlx::Error);

impl std::fmt::Debug for StoreTokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl std::error::Error for StoreTokenError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

impl std::fmt::Display for StoreTokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "A database error was encountered while trying to store a subscription"
        )
    }
}

