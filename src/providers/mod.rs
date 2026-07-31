pub mod deepseek;
pub mod openrouter;

use std::time::Duration;

use anyhow::{anyhow, bail, Context};
use serde::{Deserialize, Serialize};
use serde::de::DeserializeOwned;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceInfo {
    pub currency: String,
    pub total_balance: String,
    pub granted_balance: String,
    pub topped_up_balance: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceReport {
    pub is_available: bool,
    pub balance_infos: Vec<BalanceInfo>,
}

pub trait Provider {
    fn api_key_env_name(&self) -> &'static str;
    fn fetch_balance(&self, api_key: &str) -> Result<BalanceReport, anyhow::Error>;
}

#[derive(Debug, Deserialize)]
struct ApiErrorBody {
    error: ApiError,
}

#[derive(Debug, Deserialize)]
struct ApiError {
    message: String,
}

pub(crate) fn get_json<T: DeserializeOwned>(url: &str, api_key: &str) -> Result<T, anyhow::Error> {
    let resp = match ureq::get(url)
        .timeout(DEFAULT_TIMEOUT)
        .set("Authorization", &format!("Bearer {api_key}"))
        .call()
    {
        Ok(resp) => resp,
        Err(ureq::Error::Status(code, resp)) => {
            let text = resp.into_string().unwrap_or_default();
            let detail = serde_json::from_str::<ApiErrorBody>(&text)
                .ok()
                .map(|body| body.error.message)
                .unwrap_or(text);
            bail!("接口返回 {code}: {detail}");
        }
        Err(e) => return Err(e).with_context(|| format!("请求 {url} 失败")),
    };
    let text = resp
        .into_string()
        .with_context(|| format!("读取 {url} 响应失败"))?;
    serde_json::from_str::<T>(&text).map_err(|e| anyhow!("解析响应失败: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const RESPONSE: &str = r#"{
        "is_available": true,
        "balance_infos": [
            {
                "currency": "CNY",
                "total_balance": "110.00",
                "granted_balance": "10.00",
                "topped_up_balance": "100.00"
            }
        ]
    }"#;

    #[test]
    fn parses_balance_response() {
        let report: BalanceReport = serde_json::from_str(RESPONSE).unwrap();
        assert!(report.is_available);
        assert_eq!(report.balance_infos.len(), 1);
        let info = &report.balance_infos[0];
        assert_eq!(info.currency, "CNY");
        assert_eq!(info.total_balance, "110.00");
        assert_eq!(info.granted_balance, "10.00");
        assert_eq!(info.topped_up_balance, "100.00");
    }
}
