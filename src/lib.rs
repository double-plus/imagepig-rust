use base64::Engine;
use chrono::{DateTime, Duration};
use reqwest::{Client, Error as ReqwestError};
use serde::Deserialize;
use serde_json::Value;
use std::fs::File;
use std::io::Write;
use std::time::Duration as StdDuration;
use thiserror::Error;
use url::Url;

const DOWNLOAD_ATTEMPTS: u8 = 10;
const DOWNLOAD_INTERRUPTION: u64 = 1;

#[derive(Debug, Error)]
pub enum ImagePigError {
    #[error("HTTP request failed: {0}")]
    HttpError(ReqwestError),
    #[error("Invalid URL: {0}")]
    InvalidUrl(String),
    #[error("Unexpected response")]
    UnexpectedResponse,
    #[error("Unable to fetch image")]
    MissingData,
    #[error("Cannot encode file to base64")]
    InvalidInput,
}

#[derive(Deserialize, Debug)]
pub struct APIResponse {
    content: serde_json::Value,
}

impl APIResponse {
    pub async fn data(&self) -> Result<Vec<u8>, ImagePigError> {
        if let Some(data) = self.content.get("image_data") {
            if let Some(data_str) = data.as_str() {
                return base64::prelude::BASE64_STANDARD
                    .decode(data_str)
                    .map_err(|_| ImagePigError::UnexpectedResponse);
            }
        }

        if let Some(url) = self.url() {
            for _ in 0..DOWNLOAD_ATTEMPTS {
                let response = Client::new()
                    .get(url.to_string())
                    .header("User-Agent", "Mozilla/5.0")
                    .send()
                    .await;
                if let Ok(resp) = response {
                    if resp.status().is_success() {
                        return resp
                            .bytes()
                            .await
                            .map(|b| b.to_vec())
                            .map_err(ImagePigError::HttpError);
                    }

                    if resp.status().as_u16() == 404 {
                        tokio::time::sleep(StdDuration::from_secs(DOWNLOAD_INTERRUPTION)).await;
                    } else {
                        break;
                    }
                }
            }
        }

        Err(ImagePigError::MissingData)
    }

    pub fn url(&self) -> Option<String> {
        self.content
            .get("image_url")
            .and_then(|url| url.as_str().map(|s| s.to_string()))
    }

    pub fn seed(&self) -> Option<u64> {
        self.content
            .get("seed")
            .and_then(|seed| seed.as_u64().map(|s| s as u64))
    }

    pub fn mime_type(&self) -> Option<String> {
        self.content
            .get("mime_type")
            .and_then(|mime| mime.as_str().map(|s| s.to_string()))
    }

    pub fn duration(&self) -> Option<Duration> {
        if let (Some(started), Some(completed)) = (
            self.content.get("started_at"),
            self.content.get("completed_at"),
        ) {
            let started_at = DateTime::parse_from_rfc3339(started.as_str()?).ok()?;
            let completed_at = DateTime::parse_from_rfc3339(completed.as_str()?).ok()?;

            return Some(completed_at.signed_duration_since(started_at));
        }
        None
    }

    pub async fn save(&self, path: &str) -> Result<(), ImagePigError> {
        let data = self.data().await?;
        let mut file = File::create(path).map_err(|_| ImagePigError::UnexpectedResponse)?;
        file.write_all(&data)
            .map_err(|_| ImagePigError::UnexpectedResponse)?;
        Ok(())
    }
}

#[derive(Debug)]
pub enum Proportion {
    Landscape,
    Portrait,
    Square,
    Wide,
}

impl std::fmt::Display for Proportion {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", format!("{:?}", self).to_lowercase())
    }
}

#[derive(Debug)]
pub enum UpscalingFactor {
    Two = 2,
    Four = 4,
    Eight = 8,
}

pub trait Image {
    fn prepare_image(
        &self,
        param_name: &str,
        params: &mut serde_json::Map<String, Value>,
    ) -> Result<(), ImagePigError>;
}

impl Image for &str {
    fn prepare_image(
        &self,
        param_name: &str,
        params: &mut serde_json::Map<String, Value>,
    ) -> Result<(), ImagePigError> {
        if Url::parse(self).is_ok() {
            params.insert(
                format!("{}_url", param_name),
                serde_json::Value::from(self.to_string()),
            );
            return Ok(());
        }

        return Err(ImagePigError::InvalidUrl(self.to_string()));
    }
}

impl Image for Vec<u8> {
    fn prepare_image(
        &self,
        param_name: &str,
        params: &mut serde_json::Map<String, Value>,
    ) -> Result<(), ImagePigError> {
        params.insert(
            format!("{}_data", param_name),
            serde_json::Value::from(
                base64::prelude::BASE64_STANDARD
                    .decode(self)
                    .map_err(|_| ImagePigError::InvalidInput)?,
            ),
        );
        Ok(())
    }
}

#[derive(Debug)]
pub struct ImagePig {
    api_key: String,
    api_url: String,
    client: Client,
}

impl ImagePig {
    pub fn new(api_key: String, api_url: Option<String>) -> Self {
        let api_url = api_url.unwrap_or_else(|| "https://api.imagepig.com".to_string());
        Self {
            api_key,
            api_url,
            client: Client::new(),
        }
    }

    async fn call_api(
        &self,
        endpoint: &str,
        payload: serde_json::Map<String, Value>,
    ) -> Result<APIResponse, ImagePigError> {
        let url = format!("{}/{}", self.api_url, endpoint);
        let response = self
            .client
            .post(url)
            .header("Api-Key", &self.api_key)
            .json(&payload)
            .send()
            .await
            .map_err(ImagePigError::HttpError)?;

        let content = response
            .json()
            .await
            .map_err(|_| ImagePigError::UnexpectedResponse)?;
        Ok(APIResponse { content })
    }

    pub async fn default(
        &self,
        prompt: &str,
        negative_prompt: Option<&str>,
        extra_params: Option<serde_json::Map<String, Value>>,
    ) -> Result<APIResponse, ImagePigError> {
        let mut params = extra_params.unwrap_or_default();
        params.insert(
            "positive_prompt".to_string(),
            serde_json::Value::from(prompt),
        );
        params.insert(
            "negative_prompt".to_string(),
            serde_json::Value::from(negative_prompt.unwrap_or_default()),
        );
        self.call_api("", params).await
    }

    pub async fn xl(
        &self,
        prompt: &str,
        negative_prompt: Option<&str>,
        extra_params: Option<serde_json::Map<String, Value>>,
    ) -> Result<APIResponse, ImagePigError> {
        let mut params = extra_params.unwrap_or_default();
        params.insert(
            "positive_prompt".to_string(),
            serde_json::Value::from(prompt),
        );
        params.insert(
            "negative_prompt".to_string(),
            serde_json::Value::from(negative_prompt.unwrap_or_default()),
        );
        self.call_api("xl", params).await
    }

    pub async fn flux(
        &self,
        prompt: &str,
        proportion: Option<Proportion>,
        extra_params: Option<serde_json::Map<String, Value>>,
    ) -> Result<APIResponse, ImagePigError> {
        let mut params = extra_params.unwrap_or_default();
        params.insert(
            "positive_prompt".to_string(),
            serde_json::Value::from(prompt),
        );
        params.insert(
            "proportion".to_string(),
            serde_json::Value::from(proportion.unwrap_or(Proportion::Landscape).to_string()),
        );
        self.call_api("flux", params).await
    }

    pub async fn faceswap<T: Image>(
        &self,
        source_image: T,
        target_image: T,
        extra_params: Option<serde_json::Map<String, Value>>,
    ) -> Result<APIResponse, ImagePigError> {
        let mut params = extra_params.unwrap_or_default();
        source_image
            .prepare_image("source_image", &mut params)
            .unwrap();
        target_image
            .prepare_image("target_image", &mut params)
            .unwrap();
        self.call_api("faceswap", params).await
    }

    pub async fn upscale<T: Image>(
        &self,
        image: T,
        factor: Option<UpscalingFactor>,
        extra_params: Option<serde_json::Map<String, Value>>,
    ) -> Result<APIResponse, ImagePigError> {
        let mut params = extra_params.unwrap_or_default();
        image.prepare_image("image", &mut params).unwrap();
        params.insert(
            "upscaling_factor".to_string(),
            serde_json::Value::from(factor.unwrap_or(UpscalingFactor::Two) as u8),
        );
        self.call_api("upscale", params).await
    }

    pub async fn cutout<T: Image>(
        &self,
        image: T,
        extra_params: Option<serde_json::Map<String, Value>>,
    ) -> Result<APIResponse, ImagePigError> {
        let mut params = extra_params.unwrap_or_default();
        image.prepare_image("image", &mut params).unwrap();
        self.call_api("cutout", params).await
    }

    pub async fn replace<T: Image>(
        &self,
        image: T,
        select_prompt: &str,
        positive_prompt: &str,
        negative_prompt: Option<&str>,
        extra_params: Option<serde_json::Map<String, Value>>,
    ) -> Result<APIResponse, ImagePigError> {
        let mut params = extra_params.unwrap_or_default();
        image.prepare_image("image", &mut params).unwrap();
        params.insert(
            "select_prompt".to_string(),
            serde_json::Value::from(select_prompt),
        );
        params.insert(
            "positive_prompt".to_string(),
            serde_json::Value::from(positive_prompt),
        );
        params.insert(
            "negative_prompt".to_string(),
            serde_json::Value::from(negative_prompt.unwrap_or_default()),
        );
        self.call_api("replace", params).await
    }

    pub async fn outpaint<T: Image>(
        &self,
        image: T,
        positive_prompt: &str,
        negative_prompt: Option<&str>,
        top: Option<u32>,
        right: Option<u32>,
        bottom: Option<u32>,
        left: Option<u32>,
        extra_params: Option<serde_json::Map<String, Value>>,
    ) -> Result<APIResponse, ImagePigError> {
        let mut params = extra_params.unwrap_or_default();
        image.prepare_image("image", &mut params).unwrap();
        params.insert(
            "positive_prompt".to_string(),
            serde_json::Value::from(positive_prompt),
        );
        params.insert(
            "negative_prompt".to_string(),
            serde_json::Value::from(negative_prompt.unwrap_or_default()),
        );
        params.insert(
            "top".to_string(),
            serde_json::Value::from(top.unwrap_or_default()),
        );
        params.insert(
            "right".to_string(),
            serde_json::Value::from(right.unwrap_or_default()),
        );
        params.insert(
            "bottom".to_string(),
            serde_json::Value::from(bottom.unwrap_or_default()),
        );
        params.insert(
            "left".to_string(),
            serde_json::Value::from(left.unwrap_or_default()),
        );
        self.call_api("outpaint", params).await
    }
}
