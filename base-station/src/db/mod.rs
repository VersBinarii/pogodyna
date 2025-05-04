use std::pin::Pin;

use sqlx::SqlitePool;

use crate::{SensorReading, error::BsError};

pub trait Repository: Send + Sync {
    fn insert_sensor_reading(
        &self,
        sensor_reading: SensorReading,
    ) -> Pin<Box<dyn Future<Output = Result<(), BsError>> + Send>>;
}

#[derive(Debug, Clone)]
pub struct SqliteRepository {
    pool: SqlitePool,
}

impl SqliteRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

impl Repository for SqliteRepository {
    fn insert_sensor_reading(
        &self,
        reading: SensorReading,
    ) -> Pin<Box<dyn Future<Output = Result<(), BsError>> + Send>> {
        let pool = self.pool.clone();
        Box::pin(async move {
            sqlx::query!(
            "INSERT INTO sensor_readings (sensor_id, timestamp, temperature, pressure, humidity) 
        VALUES (?,?,?,?,?)",
            reading.sensor_id,
            reading.timestamp,
            reading.temperature,
            reading.pressure,
            reading.humidity
        )
            .execute(&pool)
            .await?;

            Ok(())
        })
    }
}
