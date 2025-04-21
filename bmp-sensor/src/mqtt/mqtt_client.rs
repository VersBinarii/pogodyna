use defmt::{debug, error};
use embassy_futures::select::{select, Either};
use embassy_net::{tcp::TcpSocket, IpEndpoint};
use embassy_time::{Duration, Timer};
use embedded_io_async::{Read, Write};
use heapless::String;
use mqttrs::{
    decode_slice, encode_slice, Connack, Connect, ConnectReturnCode, Packet, Protocol, Publish,
    QosPid,
};

const MAX_SENSOR_ID_LEN: usize = 64;

use crate::error::SensorError;

pub struct MqttClient<'a> {
    socket: TcpSocket<'a>,
    broker_address: IpEndpoint,
    sensor_id: String<MAX_SENSOR_ID_LEN>,
}

impl<'a> MqttClient<'a> {
    pub fn new<T>(socket: TcpSocket<'a>, broker_address: T, id: &str) -> Self
    where
        T: Into<IpEndpoint>,
    {
        assert!(id.len() < MAX_SENSOR_ID_LEN);
        let mut sensor_id = String::new();
        let _ = sensor_id.push_str(id);
        Self {
            socket,
            broker_address: broker_address.into(),
            sensor_id,
        }
    }

    pub async fn connect(&mut self) -> Result<(), SensorError> {
        inner_connect(
            &mut self.socket,
            self.broker_address,
            self.sensor_id.as_str(),
        )
        .await
        .map(|_| ())
    }

    pub async fn publish(&mut self, topic: &str, data: &[u8]) -> Result<(), SensorError> {
        let packet: Packet = Publish {
            dup: false,
            qospid: QosPid::AtMostOnce,
            retain: false,
            topic_name: topic,
            payload: data,
        }
        .into();

        let mut buf = [0u8; 256];
        match encode_slice(&packet, &mut buf) {
            Err(e) => {
                log_mqtt_error(e);
                return Err(SensorError::Protocol);
            }
            Ok(packet_size) => match self.socket.write_all(&buf[..packet_size]).await {
                Ok(_) => Ok(()),
                Err(_) => Err(SensorError::Network),
            },
        }
    }

    pub async fn disconnect(&mut self) -> Result<(), SensorError> {
        let _ = self
            .socket
            .flush()
            .await
            .map_err(|_| SensorError::Network)?;
        self.socket.abort();
        Ok(())
    }
}

fn log_mqtt_error(e: mqttrs::Error) {
    match e {
        mqttrs::Error::WriteZero => error!("Not enough space in buffer"),
        mqttrs::Error::InvalidPid => error!("Invalid PID"),
        mqttrs::Error::InvalidHeader => error!("Invalid header"),
        mqttrs::Error::InvalidLength => error!("Invalid length"),
        _ => error!("Other error"),
    }
}

async fn inner_connect(
    socket: &mut TcpSocket<'_>,
    broker_address: IpEndpoint,
    sensor_id: &str,
) -> Result<(), SensorError> {
    match socket.connect(broker_address).await {
        Ok(_) => {
            let packet: Packet = Connect {
                protocol: Protocol::MQTT311,
                keep_alive: 120,
                client_id: sensor_id,
                clean_session: true,
                last_will: None,
                username: None,
                password: None,
            }
            .into();

            let mut buf = [0u8; 64];
            let encoded_bytes = match encode_slice(&packet, &mut buf) {
                Err(e) => {
                    log_mqtt_error(e);
                    return Err(SensorError::Protocol);
                }
                Ok(bytes) => bytes,
            };

            if let Err(_) = socket.write_all(&buf[..encoded_bytes]).await {
                return Err(SensorError::Network);
            }
            // CONNACK is small (2-byte fixed header + 2-byte variable header)
            let mut read_buf = [0u8; 4];
            let read_fut = socket.read_exact(&mut read_buf);
            let timeout_fut = Timer::after(Duration::from_secs(5));

            match select(read_fut, timeout_fut).await {
                Either::First(Ok(_)) => match decode_slice(&read_buf) {
                    Ok(Some(Packet::Connack(Connack { code, .. }))) => {
                        debug!("Received Connack");
                        if code == ConnectReturnCode::Accepted {
                            debug!("Connack accepted");
                            Ok(())
                        } else {
                            debug!("Connack rejected");
                            Err(SensorError::Protocol)
                        }
                    }
                    _ => {
                        debug!("Failed to decode what should have been a connack");
                        Err(SensorError::Protocol)
                    }
                },
                Either::First(Err(_)) => {
                    debug!("Failed to read data from socket");
                    Err(SensorError::Network)
                }
                Either::Second(_) => {
                    debug!("Timeout waiting for connack");
                    Err(SensorError::Timeout)
                }
            }
        }
        Err(_) => Err(SensorError::Network),
    }
}
