use chrono::Utc;

pub mod api;
pub mod db;
pub mod error;
pub mod mqtt;

#[derive(Debug, serde::Deserialize)]
pub struct SensorReadingEvent {
    #[serde(default = "default_sensor")]
    sensor_id: String,
    #[serde(rename = "t", with = "from_string_or_float")]
    temperature: f64,
    #[serde(rename = "p", with = "from_string_or_float")]
    pressure: f64,
    #[serde(rename = "h", with = "from_string_or_float")]
    humidity: f64,
    #[serde(default = "default_timestamp")]
    timestamp: chrono::DateTime<chrono::Utc>,
}

impl std::fmt::Display for SensorReadingEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}[{}] - temperature: {}, pressure: {}, humidity: {}",
            self.sensor_id,
            self.timestamp.to_rfc3339(),
            self.temperature,
            self.pressure,
            self.humidity
        )
    }
}

fn default_sensor() -> String {
    "outside-sensor".to_string()
}

fn default_timestamp() -> chrono::DateTime<Utc> {
    Utc::now()
}

mod from_string_or_float {
    use serde::{self, Deserialize, Deserializer};

    pub fn deserialize<'de, D>(deserializer: D) -> Result<f64, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Debug, Deserialize)]
        #[serde(untagged)]
        enum FloatOrString {
            String(String),
            Float(f64),
            Number(i64),
        }
        let maybe_value: Option<FloatOrString> = Option::deserialize(deserializer)?;
        match maybe_value {
            Some(FloatOrString::String(as_string)) => {
                let as_float: f64 = as_string.parse().map_err(|e| {
                    tracing::error!("Failed to parse string to float: {e}");
                    serde::de::Error::custom(e)
                })?;
                Ok(as_float)
            }
            Some(FloatOrString::Float(f)) => Ok(f),
            Some(FloatOrString::Number(f)) => Ok(f as f64),
            None => Err(serde::de::Error::custom("Field is missing")),
        }
    }
}
