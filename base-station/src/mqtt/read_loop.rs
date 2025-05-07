use mqttrs::{Packet, decode_slice};
use tracing::debug;

use crate::{SensorReading, db::Repository, error::BsError};

use super::ReadLoopResult;

pub async fn handle_packet(
    repository: &impl Repository,
    packet: &[u8],
) -> Result<ReadLoopResult, BsError> {
    if is_mqtt_packet(packet[0]) {
        match decode_slice(packet) {
            Ok(Some(Packet::Publish(publish))) => {
                let sensor_reading: SensorReading =
                    serde_json::from_slice(publish.payload).unwrap();
                debug!("Got update: {sensor_reading}");
                repository
                    .insert_sensor_reading(sensor_reading)
                    .await
                    .map(|_| ReadLoopResult::Ok)
            }
            _ => Ok(ReadLoopResult::Skipped),
        }
    } else {
        Ok(ReadLoopResult::Unknown)
    }
}

fn is_mqtt_packet(first_byte: u8) -> bool {
    let packet_type = first_byte >> 4;
    (1..=14).contains(&packet_type)
}

#[cfg(test)]
mod tests {
    use sqlx::SqlitePool;

    use crate::db::SqliteRepository;

    use super::*;

    const MQTT_PUBLISH_PACKET: [u8; 58] = [
        // Fixed header
        0x30, 0x38, // PUBLISH, Remaining Length = 56
        // Variable header: topic "sensor/data"
        0x00, 0x0B, // Topic length = 11
        0x73, 0x65, 0x6E, 0x73, 0x6F, 0x72, 0x2F, 0x64, 0x61, 0x74, 0x61,
        // Payload: {"t":"21.1111","p":"22.2222","h":"23.3333"}
        0x7B, 0x22, 0x74, 0x22, 0x3A, 0x22, 0x32, 0x31, 0x2E, 0x31, 0x31, 0x31, 0x31, 0x22, 0x2C,
        0x22, 0x70, 0x22, 0x3A, 0x22, 0x32, 0x32, 0x2E, 0x32, 0x32, 0x32, 0x32, 0x22, 0x2C, 0x22,
        0x68, 0x22, 0x3A, 0x22, 0x32, 0x33, 0x2E, 0x33, 0x33, 0x33, 0x33, 0x22, 0x7D,
    ];

    #[sqlx::test(migrations = "../spec/migrations")]
    async fn handle_valid_publish_packet(pool: SqlitePool) {
        let repo = SqliteRepository::new(pool.clone());
        let res = handle_packet(&repo, &MQTT_PUBLISH_PACKET).await;

        assert!(res.is_ok());
        assert_eq!(res.unwrap(), ReadLoopResult::Ok);

        let res = sqlx::query!("select humidity from sensor_readings where temperature = 21.1111")
            .fetch_one(&pool)
            .await;

        assert_eq!(23.3333, res.unwrap().humidity);
    }
}
