use crate::api::image_api::{ImageApi, ImageApiImpl};
use mbase::dependencies::{env, Env};

pub fn image_api() -> impl ImageApi {
    image_api_for_env(&env())
}

pub fn image_api_for_env(env: &Env) -> impl ImageApi {
    let host = match env {
        Env::Local => "http://localhost:8000",
        Env::Test => "http://18.214.98.83:8000",
    };
    ImageApiImpl::new(host)
}
