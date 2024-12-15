use zero2prod::telemetry::{get_subscriber, init_subscriber};

use zero2prod::configuration::get_configuration;
use zero2prod::startup::Application;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
    let subscriber = get_subscriber("zero2prod".into(), "info".into(), std::io::stdout);
    init_subscriber(subscriber);

    let configuration = get_configuration().expect("Failed to read config");
    let server = Application::build(configuration).await?;
    server.run_until_stopped().await?;
    Ok(())
}
