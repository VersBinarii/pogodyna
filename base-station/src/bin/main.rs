
use base_station::{db::SqliteRepository, error::BsError, mqtt::MqttClient};
use sqlx::SqlitePool;

#[tokio::main]
async fn main() -> Result<(), BsError> {
    dotenvy::from_filename("../.env").ok();

    let broker_ip = dotenvy::var("BASE_STATION_ADDRESS")?;
    let broker_port = dotenvy::var("BASE_STATION_PORT")?;
    let broker_addr = format!("{broker_ip}:{broker_port}").parse().unwrap();
    let db_pool = SqlitePool::connect("").await.unwrap();
    let repository = SqliteRepository::new(db_pool);
    let (mqtt_client, handle) =
        MqttClient::run_forever(broker_addr, "base-station".to_string(), repository).await;

    println!("waiting for server setup");
    mqtt_client.wait_for_server_setup().await;

    println!("sending subscription");
    if let Err(e) = mqtt_client.subscribe("sensor/update").await {
        eprintln!("Error sending subscribe: {}", e);
    }

    handle.await?;
    Ok(())
}
