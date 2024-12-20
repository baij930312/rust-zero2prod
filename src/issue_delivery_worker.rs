use sqlx::{PgPool, Postgres, Transaction};
use std::time::Duration;
use tracing::{field::display, Span};

use uuid::Uuid;

use crate::{
    configuration::Settings,
    domain::SubscriberEmail,
    email_client::EmailClient,
    idempotency::{delete_all_idempotencys, delete_expire_idempotencys},
    startup::get_connection_pool,
};

pub enum ExecutionOutcome {
    TaskCompleted,
    EmptyQueue,
}

#[tracing::instrument( skip_all,fields(newsletter_issues_id=tracing::field::Empty,subscriber_email=tracing::field::Empty),err)]
pub async fn try_execute_task(
    pool: &PgPool,
    email_client: &EmailClient,
) -> Result<ExecutionOutcome, anyhow::Error> {
    let tasks = dequeue_task(pool).await?;
    if tasks.is_none() {
        return Ok(ExecutionOutcome::EmptyQueue);
    }
    let (transaction, issue_id, email) = tasks.unwrap();

    Span::current()
        .record("newsletter_issues_id", &display(issue_id))
        .record("subscriber_email", &display(&email));

    match SubscriberEmail::parse(email.clone()) {
        Ok(email) => {
            let issue = get_issue(issue_id, pool).await?;
            if let Err(e) = email_client
                .send_email(
                    &email,
                    &issue.title,
                    &issue.html_content,
                    &issue.text_content,
                )
                .await
            {
                tracing::error!(
                    error.cause = ?e,
                    error.message = %e,
                    "Failed to deliver issue to a confirmed subscriber skipping.",
                )
            }
        }
        Err(e) => {
            tracing::error!(
                error.cause = ?e,
                error.message = %e,
                "Skipping a confirmed subscriber.\
                Their stored contant detail are invalid",
            )
        }
    }

    delete_task(issue_id, transaction, &email).await?;

    Ok(ExecutionOutcome::TaskCompleted)
}

type PgTransaction = Transaction<'static, Postgres>;

#[tracing::instrument(skip_all)]
async fn dequeue_task(
    pool: &PgPool,
) -> Result<Option<(PgTransaction, Uuid, String)>, anyhow::Error> {
    let mut transaction = pool.begin().await?;
    let r = sqlx::query!(
        r#"
        SELECT newsletter_issues_id , subscriber_email 
        FROM issues_delivery_queue
        FOR UPDATE
        SKIP LOCKED
        LIMIT 1
    "#,
    )
    .fetch_optional(&mut transaction)
    .await?;
    if let Some(r) = r {
        Ok(Some((
            transaction,
            r.newsletter_issues_id,
            r.subscriber_email,
        )))
    } else {
        Ok(None)
    }
}

#[tracing::instrument(skip_all)]
async fn delete_task(
    issue_id: Uuid,
    mut transaction: PgTransaction,
    email: &str,
) -> Result<(), anyhow::Error> {
    sqlx::query!(
        r#"
        DELETE FROM issues_delivery_queue
        WHERE
            newsletter_issues_id = $1 AND subscriber_email = $2
    "#,
        issue_id,
        email
    )
    .execute(&mut transaction)
    .await?;

    transaction.commit().await?;
    Ok(())
}

struct NewsletterIssue {
    title: String,
    text_content: String,
    html_content: String,
}

#[tracing::instrument(skip_all)]
async fn get_issue(issue_id: Uuid, pool: &PgPool) -> Result<NewsletterIssue, anyhow::Error> {
    let issue = sqlx::query_as!(
        NewsletterIssue,
        r#"
        SELECT title ,text_content,html_content
        FROM newsletter_issues
        WHERE
            newsletter_issues_id = $1
        "#,
        issue_id,
    )
    .fetch_one(pool)
    .await?;
    Ok(issue)
}

async fn worker_loop(pool: PgPool, email_client: EmailClient) -> Result<(), anyhow::Error> {
    loop {
        match try_execute_task(&pool, &email_client).await {
            Ok(ExecutionOutcome::EmptyQueue) => {
                let _ = delete_all_idempotencys(&pool).await;
                tokio::time::sleep(Duration::from_secs(10)).await
            }
            Ok(ExecutionOutcome::TaskCompleted) => {}
            Err(_) => tokio::time::sleep(Duration::from_secs(1)).await,
        }
    }
}

async fn clear_idempotencys_loop(pool: PgPool) -> Result<(), anyhow::Error> {
    loop {
        let _ = delete_expire_idempotencys(&pool).await;
        tokio::time::sleep(Duration::from_secs(300)).await
    }
}

pub async fn run_worker_until_stopped(configuration: Settings) -> Result<(), anyhow::Error> {
    let connection_pool = get_connection_pool(&configuration.database);
    let email_client = configuration.email_client.client();
    worker_loop(connection_pool, email_client).await
}

pub async fn run_clear_until_stopped(configuration: Settings) -> Result<(), anyhow::Error> {
    let connection_pool = get_connection_pool(&configuration.database);
    clear_idempotencys_loop(connection_pool).await
}
