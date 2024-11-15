use std::net::TcpListener;

use sqlx::{Connection, PgConnection};
use zero2prod::configuration::get_configuration;

#[tokio::test]
async fn health_check_works() {
    let address = spawn_app();

    let client = reqwest::Client::new();

    let response = client
        .get(&format!("{}/health_check", &address))
        .send()
        .await
        .unwrap();

    assert!(response.status().is_success());
    assert_eq!(Some(0), response.content_length());
}

#[tokio::test]
async fn subcribe_returns_a_200_for_valid_from_data() {
    let address = spawn_app();

    let config = get_configuration().expect("Failed to read config");
    let connection_string = config.database.connection_string();
    let mut connection = PgConnection::connect(&connection_string)
        .await
        .expect("Failded to connect to db");

    let client = reqwest::Client::new();
    let body = "name=bai%20jin&email=baij930312@163.com";
    let response = client
        .post(&format!("{}/subscriptions", &address))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .unwrap();

    assert_eq!(200, response.status().as_u16());
    let saved = sqlx::query!("SELECT email, name FROM subscriptions",)
        .fetch_one(&mut connection)
        .await
        .expect("Faild to fetch saved subscription");

    assert_eq!(saved.email, "baij930312@163.com");
    assert_eq!(saved.name, "bai jin");
}

#[tokio::test]
async fn subcribe_returns_a_400_when_a_data_is_missing() {
    let address = spawn_app();
    let client = reqwest::Client::new();
    let test_case = vec![
        ("name=bai", "missing the email"),
        ("email=baij930312@163.com", "missing the name"),
        ("", "missing both name and email"),
    ];
    for (body, msg) in test_case {
        let response = client
            .post(&format!("{}/subscriptions", &address))
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

fn spawn_app() -> String {
    let listener: TcpListener = TcpListener::bind("127.0.0.1:0").expect("Faild bind address");
    let port = listener.local_addr().unwrap().port();

    let server = zero2prod::startup::run(listener).expect("Failed to bind address");
    let _ = tokio::spawn(server);
    format!("http://127.0.0.1:{}", port)
}
