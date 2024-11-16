use sqlx::PgPool;
use std::net::TcpListener;
use zero2prod::configuration::get_configuration;
use zero2prod::startup::run;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let config = get_configuration().expect("Failed to read config");
    let address = format!("127.0.0.1:{}", config.app_port);
    let listener: TcpListener = TcpListener::bind(address).expect("Faild bind address");
    let config = get_configuration().expect("Failed to read config");
    let connection_string = config.database.connection_string();
    let connection_poll = PgPool::connect(&connection_string)
        .await
        .expect("Failded to connect to db");
    run(listener, connection_poll)?.await
}
