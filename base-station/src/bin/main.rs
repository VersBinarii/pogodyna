use base_station::{db::SqliteRepository, mqtt::MqttClient};
use sqlx::SqlitePool;

#[tokio::main]
async fn main() {
    let broker_addr = "192.168.1.200:1883".parse().unwrap();
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

    handle.await.unwrap();
}
