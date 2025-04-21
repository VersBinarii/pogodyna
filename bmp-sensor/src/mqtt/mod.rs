use mqtt_client::MqttClient;

use crate::error::SensorError;

pub mod mqtt_client;

pub enum MqttClientState {
    Disconnected,
    Connected,
}

pub struct MqttConnector<'a> {
    pub state: MqttClientState,
    pub client: MqttClient<'a>,
}

impl<'a> MqttConnector<'a> {
    pub fn is_connected(&self) -> bool {
        matches!(self.state, MqttClientState::Connected)
    }

    pub async fn reconnect(&mut self) -> Result<(), SensorError> {
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
