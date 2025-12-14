use chrono::{DateTime, Utc};
use poem_openapi::Object;
use serde::Deserialize;

#[derive(Debug, Deserialize, Object)]
pub struct QueryFilter {
    pub sensor_id: Option<String>,
    pub min_temperature: Option<f32>,
    pub max_temperature: Option<f32>,
    pub min_humidity: Option<f32>,
    pub max_humidity: Option<f32>,
    pub min_pressure: Option<f32>,
    pub max_pressure: Option<f32>,
}

#[derive(Debug, Deserialize, Object)]
pub struct Pagination {
    pub after: Option<DateTime<Utc>>,
    #[serde(default = "default_page_size")]
    pub page_size: usize,
}

fn default_page_size() -> usize{
    10
}

#[derive(Debug, Deserialize, Object)]
pub struct MeasurementQuery {
    pub filters: QueryFilter,
    pub pagination: Pagination,
    pub columns: Vec<String>,
}

impl MeasurementQuery{
    
    const ALLOWED_COLUMNS: &[&str] = &["sensor_id", "topic", "timestamp", "temperature", "humidity", "pressure"];

    pub fn are_columns_sane(&self) -> bool {
        self.columns.iter().all(|c| Self::ALLOWED_COLUMNS.contains(&c.as_str()))
    }
}
