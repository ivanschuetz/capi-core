use anyhow::Result;
use async_trait::async_trait;
use mbase::{
    api::{
        contract::Contract,
        version::{Version, VersionedTealSourceTemplate, Versions},
    },
    teal::TealSourceTemplate,
};
use reqwest::Client;

use crate::reqwest_ext::ResponseExt;

// Send + sync assumess the implementations to be stateless
// (also: we currently use this only in WASM, which is single threaded)
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
pub trait TealApi: Send + Sync {
    async fn last_versions(&self) -> Result<Versions>;
    async fn template(
        &self,
        contract: Contract,
        version: Version,
    ) -> Result<VersionedTealSourceTemplate>;
}

pub struct RemoteTealApi {
    host: String,
    client: Client,
}

impl RemoteTealApi {
    pub fn new(host: &str) -> RemoteTealApi {
        let client = reqwest::Client::new();
        RemoteTealApi {
            host: host.to_owned(),
            client,
        }
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl TealApi for RemoteTealApi {
    async fn last_versions(&self) -> Result<Versions> {
        let url = format!("{}/teal/versions", self.host);
        log::debug!("Will fetch last teal versions from: {:?}", url);
        Ok(self
            .client
            .get(url)
            .send()
            .await?
            .to_error_if_http_error()
            .await?
            .json()
            .await?)
    }

    async fn template(
        &self,
        contract: Contract,
        version: Version,
    ) -> Result<VersionedTealSourceTemplate> {
        let contract_str = match contract {
            Contract::DaoAppApproval => "approval",
            Contract::DaoAppClear => "clear",
            Contract::DaoCustomer => "customer",
        };

        let url = format!("{}/teal/{}/{}", self.host, contract_str, version.0);
        log::debug!("Will fetch teal template from: {:?}", url);

        let bytes = self
            .client
            .get(url)
            .send()
            .await?
            .to_error_if_http_error()
            .await?
            .bytes()
            .await?
            .to_vec();

        Ok(VersionedTealSourceTemplate {
            version,
            template: TealSourceTemplate(bytes),
        })
    }
}
