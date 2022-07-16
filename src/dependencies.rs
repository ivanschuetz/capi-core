use crate::{
    api::fetcher::{Fetcher, FetcherImpl},
    teal::{RemoteTealApi, TealApi},
};
use mbase::dependencies::{env, Env};

pub fn fetcher() -> impl Fetcher {
    FetcherImpl::new()
}

pub fn teal_api() -> impl TealApi {
    teal_api_for_env(&env())
}

pub fn teal_api_for_env(env: &Env) -> impl TealApi {
    let host = match env {
        Env::Local => "http://localhost:8000",
        Env::Test => "http://18.214.98.83:8000",
    };
    RemoteTealApi::new(host)
}
