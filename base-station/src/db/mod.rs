use async_trait::async_trait;
use sqlx::{QueryBuilder, Sqlite, SqlitePool};

use crate::SensorReadingEvent;
use crate::error::BsError;

mod pagination;

pub use pagination::MeasurementQuery;

#[async_trait]
pub trait Repository: Send + Sync {
    async fn insert_sensor_reading(
        &self,
        topic: String,
        sensor_reading: SensorReadingEvent,
    ) -> Result<(), BsError>;
    async fn fetch_sensor_readings_page(&self, query: MeasurementQuery) -> Result<(), BsError>;
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

#[async_trait]
impl Repository for SqliteRepository {
    async fn insert_sensor_reading(
        &self,
        topic: String,
        reading: SensorReadingEvent,
    ) -> Result<(), BsError> {
        let pool = self.pool.clone();
        sqlx::query!(
            "INSERT INTO sensor_readings (sensor_id, topic, timestamp, temperature, pressure, \
             humidity) 
                VALUES (?,?,?,?,?,?)",
            reading.sensor_id,
            topic,
            reading.timestamp,
            reading.temperature,
            reading.pressure,
            reading.humidity
        )
        .execute(&pool)
        .await?;

        Ok(())
    }

    async fn fetch_sensor_readings_page(&self, query: MeasurementQuery) -> Result<(), BsError> {
        if query.columns.is_empty() || !query.are_columns_sane() {
            return Err(BsError::Other("Invalid columns".to_string()));
        }
        let mut qb = QueryBuilder::<Sqlite>::new("SELECT ");

        for (i, col) in query.columns.iter().enumerate() {
            if i > 0 {
                qb.push(", ");
            }
            qb.push(col);
        }

        qb.push(" FROM sensor_readings");

        // WHERE clause
        let mut has_where = false;
        let mut push_and = |qb: &mut QueryBuilder<Sqlite>| {
            if has_where {
                qb.push(" AND ");
            } else {
                qb.push(" WHERE ");
                has_where = true;
            }
        };

        let f = &query.filters;

        if let Some(sensor_id) = &f.sensor_id {
            push_and(&mut qb);
            qb.push("sensor_id = ").push_bind(sensor_id);
        }

        if let Some(min_temp) = f.min_temperature {
            push_and(&mut qb);
            qb.push("temperature >= ").push_bind(min_temp);
        }

        if let Some(max_temp) = f.max_temperature {
            push_and(&mut qb);
            qb.push("temperature <= ").push_bind(max_temp);
        }

        if let Some(min_hum) = f.min_humidity {
            push_and(&mut qb);
            qb.push("humidity >= ").push_bind(min_hum);
        }

        if let Some(max_hum) = f.max_humidity {
            push_and(&mut qb);
            qb.push("humidity <= ").push_bind(max_hum);
        }

        if let Some(min_press) = f.min_pressure {
            push_and(&mut qb);
            qb.push("pressure >= ").push_bind(min_press);
        }

        if let Some(max_press) = f.max_pressure {
            push_and(&mut qb);
            qb.push("pressure <= ").push_bind(max_press);
        }

        if let Some(after) = query.pagination.after {
            push_and(&mut qb);
            qb.push("timestamp > ").push_bind(after);
        }

        qb.push(" ORDER BY timestamp ASC");
        qb.push(" LIMIT ")
            .push_bind(query.pagination.page_size as i64);

        let sql_query = qb.build();
        let rows = sql_query.fetch_all(&self.pool).await?;

        unimplemented!("Need to format the result")
    }
}
