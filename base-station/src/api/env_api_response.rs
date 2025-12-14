use poem_openapi::{payload::Json, types::ToJSON, ApiResponse, Object};
use serde::Serialize;

#[derive(Debug, Serialize, Object)]
pub struct EnvironmentApiData;

#[derive(Debug, ApiResponse)]
pub enum EnvironmentApiResponse<T: ToJSON + Send>{
    #[oai(status = 200)]
    Ok(Json<T>),
    #[oai(status = 400)]
    ClientError,
    #[oai(status = 404)]
    NotFound,
    #[oai(status = 500)]
    InternalServerError,
}
