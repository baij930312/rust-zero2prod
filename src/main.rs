use std::fmt::{Debug, Display};

use tokio::task::JoinError;
use zero2prod::issue_delivery_worker::run_worker_until_stopped;
use zero2prod::telemetry::{get_subscriber, init_subscriber};

use zero2prod::configuration::get_configuration;
use zero2prod::startup::Application;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
    let subscriber = get_subscriber("zero2prod".into(), "info".into(), std::io::stdout);
    init_subscriber(subscriber);

    let configuration = get_configuration().expect("Failed to read config");
    let server = Application::build(configuration.clone()).await?;
    let application = server.run_until_stopped();
    let worker = run_worker_until_stopped(configuration);
    tokio::select! {
        o = application => report_exit("API",Ok(o)),
        o = worker => report_exit("Background worker",Ok(o)),
    };
    Ok(())
}

fn report_exit(task_name: &str, outcome: Result<Result<(), impl Debug + Display>, JoinError>) {
    match outcome {
        Ok(Ok(())) => {
            tracing::info!("{} has exited", task_name)
        }
        Ok(Err(e)) => {
            tracing::error!(
                error.cause = ?e,
                error.message = %e,
               "{} failed", task_name
            )
        }
        Err(e) => {
            tracing::error!(
                error.cause = ?e,
                error.message = %e,
               "{} task failed to complete", task_name
            )
        }
    }
}
