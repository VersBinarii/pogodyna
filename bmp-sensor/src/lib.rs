#![no_std]

use bme280::Measurements;

#[derive(serde::Serialize)]
pub struct SensorUpdate {
    temperature: f32,
    pressure: f32,
    humidity: f32,
}

impl<E> From<Measurements<E>> for SensorUpdate {
    fn from(value: Measurements<E>) -> Self {
        Self {
            temperature: value.temperature,
            pressure: value.pressure,
            humidity: value.humidity,
        }
    }
}
