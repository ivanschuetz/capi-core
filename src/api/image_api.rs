use crate::flows::create_dao::storage::load_dao::DaoAppId;
use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;

// Send + sync assumess the implementations to be stateless
// (also: we currently use this only in WASM, which is single threaded)
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
pub trait ImageApi: Send + Sync {
    async fn upload_image(&self, app_id: DaoAppId, image: Vec<u8>) -> Result<()>;
    async fn get_image(&self, id: &str) -> Result<Vec<u8>>;
    fn image_url(&self, id: &str) -> String;
}

pub struct ImageApiImpl {
    host: String,
    client: Client,
}

impl ImageApiImpl {
    pub fn new(host: &str) -> ImageApiImpl {
        let client = reqwest::Client::new();
        ImageApiImpl {
            host: host.to_owned(),
            client,
        }
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl ImageApi for ImageApiImpl {
    async fn upload_image(&self, app_id: DaoAppId, image: Vec<u8>) -> Result<()> {
        self.client
            .post(format!("{}/{}", self.host, app_id.0))
            .body(image)
            .send()
            .await?;

        Ok(())
    }

    async fn get_image(&self, id: &str) -> Result<Vec<u8>> {
        Ok(self
            .client
            .get(self.image_url(id))
            .send()
            .await?
            .bytes()
            .await?
            .to_vec())
    }

    fn image_url(&self, id: &str) -> String {
        format!("{}/{}", self.host, id)
    }
}
