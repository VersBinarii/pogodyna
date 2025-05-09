use base_station::{api::EnvironmentApi, db::SqliteRepository, error::BsError, mqtt::MqttClient};
use poem::{Route, Server, listener::TcpListener};
use poem_openapi::OpenApiService;
use sqlx::SqlitePool;
use tracing::info;
use tracing_appender::rolling;
use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_subscriber::{fmt, layer::{Layer, SubscriberExt}, EnvFilter, Registry};

#[tokio::main]
async fn main() -> Result<(), BsError> {
    dotenvy::from_filename("../.env")?;

    let log_directory = dotenvy::var("LOG_DIRECTORY")?;
    let appender = rolling::daily(log_directory, "base_station_log.json");
    let formatting_layer = BunyanFormattingLayer::new("base-station".into(), appender);
    let console_layer = fmt::layer()
        .with_target(true)
        .with_thread_names(true)
        .with_filter(
            EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("debug"))
    );
    let subscriber = Registry::default()
        .with(JsonStorageLayer)
        .with(formatting_layer)
        .with(console_layer);

    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to set subscriber");

    let broker_ip = dotenvy::var("BASE_STATION_ADDRESS")?;
    let broker_port = dotenvy::var("BASE_STATION_PORT")?;
    let broker_addr = format!("{broker_ip}:{broker_port}").parse().unwrap();
    let sqlite_db_file = dotenvy::var("DATABASE_URL")?;
    let db_pool = SqlitePool::connect(&sqlite_db_file).await?;

    sqlx::migrate!("./migrations").run(&db_pool).await?;
    let repository = SqliteRepository::new(db_pool);
    let (mqtt_client, handle) =
        MqttClient::run_forever(broker_addr, "base-station".to_string(), repository).await;

    info!("waiting for MQTT server setup");
    mqtt_client.wait_for_server_setup().await;

    let api_service =
        OpenApiService::new(EnvironmentApi, "Environment Api", "1.0").server("http://localhost:3000");
    let ui = api_service.swagger_ui();
    let app = Route::new().nest("/", api_service).nest("/meta/swagger", ui);

    Server::new(TcpListener::bind("127.0.0.1:3000"))
        .run(app)
        .await?;

    handle.await?;

    Ok(())
}
