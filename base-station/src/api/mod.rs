use env_api_response::{EnvironmentApiData, EnvironmentApiResponse};
use poem::web::Json;
use poem_openapi::OpenApi;

use crate::db::{MeasurementQuery, Repository};

mod env_api_response;

pub struct EnvironmentApi<R>{
    pub repository: R
}

#[OpenApi(prefix_path = "/v1")]
impl<R> EnvironmentApi<R> where R: Repository + 'static{
    #[oai(method="get", path = "/")]
    async fn get(&self, query: Json<MeasurementQuery>)->EnvironmentApiResponse<EnvironmentApiData>{
        todo!()
    }

}
