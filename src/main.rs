use secrecy::ExposeSecret;
use sqlx::PgPool;
use std::net::TcpListener;
use zero2prod::telemetry::{get_subscriber, init_subscriber};

use zero2prod::configuration::get_configuration;
use zero2prod::startup::run;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    // env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
    let subscriber = get_subscriber("zero2prod".into(), "info".into(), std::io::stdout);
    init_subscriber(subscriber);

    let config = get_configuration().expect("Failed to read config");
    let address = format!("127.0.0.1:{}", config.app_port);
    let listener: TcpListener = TcpListener::bind(address).expect("Faild bind address");
    let config = get_configuration().expect("Failed to read config");
    let connection_poll = PgPool::connect(&config.database.connection_string().expose_secret())
        .await
        .expect("Failded to connect to db");

    run(listener, connection_poll)?.await
}
