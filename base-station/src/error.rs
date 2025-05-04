#[derive(Debug, thiserror::Error)]
pub enum BsError {
    #[error("Error communication with MQQT brocker")]
    Network(#[from] std::io::Error),
    #[error("Error processing packet: {0}")]
    Mqtt(#[from] mqttrs::Error),
    #[error("Mqtt protocol error: {0}")]
    Protocol(String),
    #[error("Processing timeout")]
    Timeout,
    #[error("Config parsing error: {0}")]
    Config(#[from] dotenvy::Error),
    #[error("Threading error: {0}")]
    Threading(#[from] tokio::task::JoinError),
    #[error("Repository error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("Database migration error: {0}")]
    Migrations(#[from] sqlx::migrate::MigrateError),
}
