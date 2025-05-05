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
