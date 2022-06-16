use crate::reqwest_ext::ResponseExt;
use anyhow::Result;
use async_trait::async_trait;
use mbase::models::dao_app_id::DaoAppId;
use reqwest::Client;

/// Image api client
// Send + sync assumess the implementations to be stateless
// (also: we currently use this only in WASM, which is single threaded)
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
pub trait ImageApi: Send + Sync {
    async fn upload_image(&self, app_id: DaoAppId, image: Vec<u8>) -> Result<()>;
    async fn get_image(&self, id: &str) -> Result<Vec<u8>>;
    fn image_url(&self, id: &str) -> String;

    async fn upload_descr(&self, app_id: DaoAppId, descr: Vec<u8>) -> Result<()>;
    async fn get_descr(&self, id: &str) -> Result<String>;
    fn descr_url(&self, id: &str) -> String;
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

    async fn upload(&self, subpath: &str, app_id: DaoAppId, bytes: Vec<u8>) -> Result<()> {
        let url = format!("{}/{subpath}/{}", self.host, app_id.0);
        log::debug!("Uploading image to: {}", url);
        self.client
            .post(url)
            .header("Content-Type", "application/octet-stream")
            .body(bytes)
            .send()
            .await?
            .to_error_if_http_error()
            .await?;
        Ok(())
    }

    async fn get(&self, url: &str) -> Result<Vec<u8>> {
        Ok(self
            .client
            .get(url)
            .send()
            .await?
            .to_error_if_http_error()
            .await?
            .bytes()
            .await?
            .to_vec())
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl ImageApi for ImageApiImpl {
    async fn upload_image(&self, app_id: DaoAppId, image: Vec<u8>) -> Result<()> {
        self.upload("image", app_id, image).await
    }

    async fn get_image(&self, id: &str) -> Result<Vec<u8>> {
        let url = self.image_url(id);
        log::debug!("Fetching image from: {}", url);
        self.get(&url).await
    }

    fn image_url(&self, id: &str) -> String {
        let encoded_id = urlencoding::encode(id).to_string();
        format!("{}/image/{}", self.host, encoded_id)
    }

    async fn upload_descr(&self, app_id: DaoAppId, image: Vec<u8>) -> Result<()> {
        self.upload("descr", app_id, image).await
    }

    async fn get_descr(&self, id: &str) -> Result<String> {
        let url = self.descr_url(id);
        log::debug!("Fetching descr from: {}", url);
        let bytes = self.get(&url).await?;
        Ok(String::from_utf8(bytes)?)
    }

    fn descr_url(&self, id: &str) -> String {
        let encoded_id = urlencoding::encode(id).to_string();
        format!("{}/descr/{}", self.host, encoded_id)
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
