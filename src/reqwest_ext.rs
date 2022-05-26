use anyhow::{anyhow, Result};
use async_trait::async_trait;
use reqwest::Response;

// reqwest::Response has thread unsafe contents with the WASM target,
// so it's required to implement Send, which is not possible.
// Since WASM is single threaded, this can be skipped, using ?Send
// https://docs.rs/async-trait/0.1.50/async_trait/#non-threadsafe-futures
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
// reqwest::Response is thread safe with non WASM targets
// async_trait doesn't need additional parameters.
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
pub trait ResponseExt {
    /// Maps error to custom error, with a possible message returned by API.
    async fn to_error_if_http_error(self) -> Result<Response>;
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl ResponseExt for Response {
    async fn to_error_if_http_error(self) -> Result<Response> {
        match self.error_for_status_ref() {
            // The response is not an error
            Ok(_) => Ok(self),
            // The response is an error
            Err(e) => Err(anyhow!("HTTP error: {e:?}")),
        }
    }
}
