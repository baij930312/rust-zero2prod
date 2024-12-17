use argon2::{password_hash::SaltString, Argon2, Params, PasswordHasher, Version};
use once_cell::sync::Lazy;
use reqwest::Url;

use sqlx::{Connection, Executor, PgConnection, PgPool};
use uuid::Uuid;
use wiremock::MockServer;
use zero2prod::{
    configuration::{get_configuration, DatabaseSettings},
    email_client::EmailClient,
    idempotency::delete_all_idempotencys,
    issue_delivery_worker::{try_execute_task, ExecutionOutcome},
    startup::{get_connection_pool, Application},
    telemetry::{get_subscriber, init_subscriber},
};

static TRACING: Lazy<()> = Lazy::new(|| {
    let default_filter_level = "info".to_string();
    let subscriber_name = "test".to_string();
    if std::env::var("TEST_LOG").is_ok() {
        let subscriber = get_subscriber(subscriber_name, default_filter_level, std::io::stdout);
        init_subscriber(subscriber);
    } else {
        let subscriber = get_subscriber(subscriber_name, default_filter_level, std::io::sink);
        init_subscriber(subscriber);
    }
});

pub struct ConfirmationLinks {
    pub html: reqwest::Url,
    pub plain_text: reqwest::Url,
}

pub struct TestApp {
    pub address: String,
    pub db_pool: PgPool,
    pub port: u16,
    pub email_server: MockServer,
    pub test_user: TestUser,
    pub app_client: reqwest::Client,
    pub email_client: EmailClient,
}

pub struct TestUser {
    pub user_id: Uuid,
    pub username: String,
    pub password: String,
}

impl TestUser {
    pub fn generate() -> Self {
        Self {
            user_id: Uuid::new_v4(),
            username: Uuid::new_v4().to_string(),
            password: Uuid::new_v4().to_string(),
        }
    }

    async fn store(&self, pool: &PgPool) {
        let salt = SaltString::generate(&mut rand::thread_rng());
        let password_hash = Argon2::new(
            argon2::Algorithm::Argon2id,
            Version::V0x13,
            Params::new(15000, 2, 1, None).unwrap(),
        )
        .hash_password(self.password.as_bytes(), &salt)
        .unwrap()
        .to_string();
        sqlx::query!(
            "INSERT INTO users (user_id,username,password_hash) VALUES ($1,$2,$3) ",
            self.user_id,
            self.username,
            password_hash
        )
        .execute(pool)
        .await
        .expect("Failed to store test user");
    }
}

impl TestApp {
    pub async fn dispatch_all_pending_emails(&self) {
        loop {
            if let ExecutionOutcome::EmptyQueue =
                try_execute_task(&self.db_pool, &self.email_client)
                    .await
                    .unwrap()
            {
                break;
            }
        }
    }

    pub async fn clear_idempotencys(&self) {
        let _ = delete_all_idempotencys(&self.db_pool).await;
    }

    pub async fn get_login_html(&self) -> String {
        self.app_client
            .get(&format!("{}/login", &self.address))
            .send()
            .await
            .expect("Failed to execute request.")
            .text()
            .await
            .unwrap()
    }

    pub async fn get_change_passwrod_html(&self) -> String {
        self.get_change_passwrod().await.text().await.unwrap()
    }

    pub async fn get_change_passwrod(&self) -> reqwest::Response {
        self.app_client
            .get(&format!("{}/admin/password", &self.address))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_admin_dashboard(&self) -> reqwest::Response {
        self.app_client
            .get(&format!("{}/admin/dashboard", &self.address))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn post_change_password<Body>(&self, body: &Body) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        self.app_client
            .post(&format!("{}/admin/password", &self.address))
            .form(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn post_logout(&self) -> reqwest::Response {
        self.app_client
            .post(&format!("{}/admin/logout", &self.address))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_admin_dashboard_html(&self) -> String {
        self.get_admin_dashboard().await.text().await.unwrap()
    }

    pub async fn post_login<Body>(&self, body: &Body) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        self.app_client
            .post(&format!("{}/login", &self.address))
            .form(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn post_subscriptions(&self, body: String) -> reqwest::Response {
        self.app_client
            .post(&format!("{}/subscriptions", &self.address))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .unwrap()
    }

    pub async fn post_newsletter<Body>(&self, body: &Body) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        self.app_client
            .post(&format!("{}/admin/newsletters", &self.address))
            .form(&body)
            .send()
            .await
            .expect("Failed to excute request")
    }

    pub async fn get_newsletter(&self) -> reqwest::Response {
        self.app_client
            .get(&format!("{}/admin/newsletters", &self.address))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_newsletter_html(&self) -> String {
        self.get_newsletter().await.text().await.unwrap()
    }

    pub fn get_confirmation_links(&self, email_request: &wiremock::Request) -> ConfirmationLinks {
        let body: serde_json::Value = serde_json::from_slice(&email_request.body).unwrap();
        let get_link = |s: &str| {
            let links: Vec<_> = linkify::LinkFinder::new()
                .links(s)
                .filter(|l| *l.kind() == linkify::LinkKind::Url)
                .collect();
            assert_eq!(links.len(), 1);
            let raw_link = links[0].as_str().to_owned();
            let mut confirmation_link = Url::parse(&raw_link).unwrap();
            assert_eq!(confirmation_link.host_str().unwrap(), "127.0.0.1");
            confirmation_link.set_port(Some(self.port)).unwrap();
            confirmation_link
        };
        let html = get_link(&body["TextBody"].as_str().unwrap());
        let plain_text = get_link(&body["TextBody"].as_str().unwrap());
        ConfirmationLinks { html, plain_text }
    }

    // pub async fn test_user(&self) -> (String, String) {
    //     let row = sqlx::query!(
    //         // Row,
    //         r#"
    //         SELECT username, password FROM users LIMIT 1
    //     "#,
    //     )
    //     .fetch_one(&self.db_pool)
    //     .await
    //     .expect("Failed to create test users.");
    //     (row.username, row.password)
    // }
}

pub async fn spawn_app() -> TestApp {
    Lazy::force(&TRACING);

    let mut config = get_configuration().expect("Failed to read config");
    let email_server = MockServer::start().await;
    config.database.database_name = Uuid::new_v4().to_string();
    config.application.port = 0;
    config.email_client.base_url = email_server.uri();
    configure_database(&config.database).await;
    let application = Application::build(config.clone()).await.unwrap();

    let address = format!("http://127.0.0.1:{}", application.port());
    let port = application.port();
    let _ = tokio::spawn(application.run_until_stopped());
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .cookie_store(true)
        .build()
        .unwrap();
    let test_app = TestApp {
        address,
        db_pool: get_connection_pool(&config.database),
        email_server,
        port,
        test_user: TestUser::generate(),
        email_client: config.email_client.client(),
        app_client: client,
    };
    test_app.test_user.store(&test_app.db_pool).await;
    test_app
}

// async fn add_test_user(pool: &PgPool) {
//     sqlx::query!(
//         // Row,
//         r#"
//             INSERT INTO users(user_id , username,password) VALUES ($1,$2,$3)
//         "#,
//         Uuid::new_v4(),
//         Uuid::new_v4().to_string(),
//         Uuid::new_v4().to_string(),
//     )
//     .execute(pool)
//     .await
//     .expect("Failed to create test users.");
// }

async fn configure_database(config: &DatabaseSettings) -> PgPool {
    let mut connection = PgConnection::connect_with(&config.without_db())
        .await
        .expect("Failded to connect to db");
    connection
        .execute(format!(r#"CREATE DATABASE "{}";"#, config.database_name).as_str())
        .await
        .expect("Failded to create table");

    let db_pool = PgPool::connect_with(config.with_db())
        .await
        .expect("Failded to connect to db");
    sqlx::migrate!("./migrations")
        .run(&db_pool)
        .await
        .expect("Failded to migrate the database");
    db_pool
}

pub fn assert_is_redirect_to(response: &reqwest::Response, location: &str) {
    assert_eq!(response.status().as_u16(), 303);
    assert_eq!(response.headers().get("Location").unwrap(), location);
}
