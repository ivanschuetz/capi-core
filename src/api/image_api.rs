use anyhow::Result;
use async_trait::async_trait;
use mbase::models::dao_app_id::DaoAppId;
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
        let url = format!("{}/image/{}", self.host, app_id.0);
        log::debug!("Uploading image to: {}", url);
        self.client
            .post(url)
            .header("Content-Type", "application/octet-stream")
            .body(image)
            .send()
            .await?;
        Ok(())
    }

    async fn get_image(&self, id: &str) -> Result<Vec<u8>> {
        let url = self.image_url(id);
        log::debug!("Fetching image from: {}", url);
        Ok(self.client.get(url).send().await?.bytes().await?.to_vec())
    }

    fn image_url(&self, id: &str) -> String {
        let encoded_id = urlencoding::encode(&id).to_string();
        format!("{}/image/{}", self.host, encoded_id)
    }
}

#[cfg(test)]
mod tests {
    use super::ImageApi;
    use crate::dependencies::image_api;
    use anyhow::Result;
    use mbase::models::dao_app_id::DaoAppId;
    use tokio::test;

    #[test]
    #[ignore]
    async fn upload_image() -> Result<()> {
        let image_api = image_api();
        image_api.upload_image(DaoAppId(123), vec![1, 2, 3]).await?;
        Ok(())
    }

    #[test]
    #[ignore]
    async fn get_image() -> Result<()> {
        let image_api = image_api();
        let image = image_api.get_image("abc").await?;
        println!("image: {:?}", image);

        Ok(())
    }
}
