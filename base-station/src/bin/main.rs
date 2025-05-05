use base_station::{db::SqliteRepository, error::BsError, mqtt::MqttClient};
use sqlx::SqlitePool;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), BsError> {
    dotenvy::from_filename("../.env")?;

    let broker_ip = dotenvy::var("BASE_STATION_ADDRESS")?;
    let broker_port = dotenvy::var("BASE_STATION_PORT")?;
    let broker_addr = format!("{broker_ip}:{broker_port}").parse().unwrap();
    let sqlite_db_file = dotenvy::var("DATABASE_URL")?;
    let db_pool = SqlitePool::connect(&sqlite_db_file).await?;

    sqlx::migrate!("../spec/migrations").run(&db_pool).await?;
    let repository = SqliteRepository::new(db_pool);
    let (mqtt_client, handle) =
        MqttClient::run_forever(broker_addr, "base-station".to_string(), repository).await;

    info!("waiting for server setup");
    mqtt_client.wait_for_server_setup().await;

    handle.await?;
    Ok(())
}
