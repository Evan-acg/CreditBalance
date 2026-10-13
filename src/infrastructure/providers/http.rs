use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use serde::Deserialize;
use serde::de::DeserializeOwned;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Deserialize)]
struct ApiErrorBody {
    error: ApiError,
}

#[derive(Debug, Deserialize)]
struct ApiError {
    message: String,
}

pub(crate) fn get_json<T: DeserializeOwned>(url: &str, api_key: &str) -> Result<T> {
    let response = match ureq::get(url)
        .timeout(DEFAULT_TIMEOUT)
        .set("Authorization", &format!("Bearer {api_key}"))
        .call()
    {
        Ok(response) => response,
        Err(ureq::Error::Status(code, response)) => {
            let text = response.into_string().unwrap_or_default();
            let detail = serde_json::from_str::<ApiErrorBody>(&text)
                .ok()
                .map(|body| body.error.message)
                .unwrap_or(text);
            bail!("接口返回 {code}: {detail}");
        }
        Err(error) => return Err(error).with_context(|| format!("请求 {url} 失败")),
    };
    let text = response
        .into_string()
        .with_context(|| format!("读取 {url} 响应失败"))?;
    serde_json::from_str::<T>(&text).map_err(|error| anyhow!("解析响应失败: {error}"))
}
