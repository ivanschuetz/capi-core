use crate::{
    api::fetcher::{Fetcher, FetcherImpl},
    teal::{RemoteTealApi, TealApi},
};

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum TealApiLocation {
    Local,
    Test,
}

pub fn fetcher() -> impl Fetcher {
    FetcherImpl::new()
}

pub fn teal_api_loc() -> TealApiLocation {
    let str = option_env!("TEAL_API");
    log::debug!("teal api location: {:?}", str);

    let env = match str {
        Some("local") => TealApiLocation::Local,
        Some("test") => TealApiLocation::Test,
        _ => {
            log::warn!("No teal api location passed to build. Defaulting to Test.");
            TealApiLocation::Test
        }
    };
    log::info!("TealApiLoc: {:?}", env);
    env
}

pub fn teal_api() -> impl TealApi {
    teal_api_for_env(&teal_api_loc())
}

pub fn teal_api_for_env(env: &TealApiLocation) -> impl TealApi {
    let host = match env {
        TealApiLocation::Local => "http://localhost:8000",
        TealApiLocation::Test => "http://143.244.177.249:8000",
    };
    RemoteTealApi::new(host)
}
