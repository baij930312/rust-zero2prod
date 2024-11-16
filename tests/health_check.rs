use std::net::TcpListener;

use sqlx::{Connection, Executor, PgConnection, PgPool};
use uuid::Uuid;
use zero2prod::configuration::{get_configuration, DatabaseSettings};

pub struct TestApp {
    pub address: String,
    pub db_pool: PgPool,
}

#[tokio::test]
async fn health_check_works() {
    let app = spawn_app().await;

    let client = reqwest::Client::new();

    let response = client
        .get(&format!("{}/health_check", &app.address))
        .send()
        .await
        .unwrap();

    assert!(response.status().is_success());
    assert_eq!(Some(0), response.content_length());
}

#[tokio::test]
async fn subcribe_returns_a_200_for_valid_from_data() {
    let app = spawn_app().await;

    let client = reqwest::Client::new();
    let body = "name=bai%20jin&email=baij930312@163.com";
    let response = client
        .post(&format!("{}/subscriptions", &app.address))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .unwrap();

    assert_eq!(200, response.status().as_u16());
    let saved = sqlx::query!("SELECT email, name FROM subscriptions",)
        .fetch_one(&app.db_pool.clone())
        .await
        .expect("Faild to fetch saved subscription");

    assert_eq!(saved.email, "baij930312@163.com");
    assert_eq!(saved.name, "bai jin");
}

#[tokio::test]
async fn subcribe_returns_a_400_when_a_data_is_missing() {
    let app = spawn_app().await;
    let client = reqwest::Client::new();
    let test_case = vec![
        ("name=bai", "missing the email"),
        ("email=baij930312@163.com", "missing the name"),
        ("", "missing both name and email"),
    ];
    for (body, msg) in test_case {
        let response = client
            .post(&format!("{}/subscriptions", &app.address))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .unwrap();
        assert_eq!(
            400,
            response.status().as_u16(),
            "The Api did not fail with 400 Bad Request when the payload was {}",
            msg
        );
    }
}

async fn spawn_app() -> TestApp {
    let mut config = get_configuration().expect("Failed to read config");
    config.database.database_name = Uuid::new_v4().to_string();
    let db_pool = configure_database(&config.database).await;

    let listener: TcpListener = TcpListener::bind("127.0.0.1:0").expect("Faild bind address");
    let port = listener.local_addr().unwrap().port();
    let address = format!("http://127.0.0.1:{}", port);
    let server =
        zero2prod::startup::run(listener, db_pool.clone()).expect("Failed to bind address");
    let _ = tokio::spawn(server);
    TestApp { address, db_pool }
}

async fn configure_database(config: &DatabaseSettings) -> PgPool {
    let mut connection = PgConnection::connect(&config.connection_string_without_db())
        .await
        .expect("Failded to connect to db");
    connection
        .execute(format!(r#"CREATE DATABASE "{}";"#, config.database_name).as_str())
        .await
        .expect("Failded to create table");

    let db_pool = PgPool::connect(&config.connection_string())
        .await
        .expect("Failded to connect to db");
    sqlx::migrate!("./migrations")
        .run(&db_pool)
        .await
        .expect("Failded to migrate the database");
    db_pool
}
