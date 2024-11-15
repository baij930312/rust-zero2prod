use std::net::TcpListener;
use zero2prod::configuration::get_configuration;
use zero2prod::startup::run;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let config = get_configuration().expect("Failed to read config");
    let address = format!("127.0.0.1:{}", config.app_port);
    let listener: TcpListener = TcpListener::bind(address).expect("Faild bind address");
    run(listener)?.await
}
