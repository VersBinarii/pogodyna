use embassy_net::{tcp::TcpSocket, IpEndpoint};
use mqtt_client::MqttClient;

use crate::error::SensorError;

mod mqtt_client;

enum MqttClientState {
    Disconnected,
    Connected,
}

pub struct MqttConnector<'a> {
    state: MqttClientState,
    client: MqttClient<'a>,
}

impl<'a> MqttConnector<'a> {
    pub fn new(socket: TcpSocket<'a>, remote_endpoint: IpEndpoint, sensor_id: &str) -> Self {
        Self {
            client: MqttClient::new(socket, remote_endpoint, sensor_id),
            state: MqttClientState::Disconnected,
        }
    }
    pub fn is_connected(&self) -> bool {
        matches!(self.state, MqttClientState::Connected)
    }

    pub async fn connect(&mut self) -> Result<(), SensorError> {
        match &mut self.state {
            MqttClientState::Disconnected => {
                let _ = self.client.connect().await?;
                self.state = MqttClientState::Connected;
                Ok(())
            }
            MqttClientState::Connected => Ok(()),
        }
    }

    pub async fn publish(&mut self, topic: &str, data: &[u8]) -> Result<(), SensorError> {
        match self.state {
            MqttClientState::Connected => {
                match self.client.publish(topic, data).await {
                    Ok(()) => Ok(()),
                    Err(_) => {
                        // On failure, attempt disconnect
                        self.client.disconnect().await?;
                        self.state = MqttClientState::Disconnected;
                        Err(SensorError::Disconnected)
                    }
                }
            }
            MqttClientState::Disconnected => Err(SensorError::Disconnected),
        }
    }
}
