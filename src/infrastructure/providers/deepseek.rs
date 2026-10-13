use serde::Deserialize;

use super::{http::get_json, read_api_key};
use crate::application::ports::BalanceProvider;
use crate::domain::balance::{BalanceInfo, BalanceReport, DEEPSEEK_PROVIDER_ID, ProviderId};

pub struct DeepSeek;

const API_KEY_ENV: &str = "DS_BALANCE_API_KEY";
const BASE_URL: &str = "https://api.deepseek.com";

#[derive(Debug, Deserialize)]
struct BalanceResponse {
    is_available: bool,
    balance_infos: Vec<BalanceInfoResponse>,
}

#[derive(Debug, Deserialize)]
struct BalanceInfoResponse {
    currency: String,
    total_balance: String,
    granted_balance: String,
    topped_up_balance: String,
}

impl From<BalanceResponse> for BalanceReport {
    fn from(response: BalanceResponse) -> Self {
        Self {
            is_available: response.is_available,
            balance_infos: response
                .balance_infos
                .into_iter()
                .map(|info| BalanceInfo {
                    currency: info.currency,
                    total_balance: info.total_balance,
                    granted_balance: info.granted_balance,
                    topped_up_balance: info.topped_up_balance,
                })
                .collect(),
        }
    }
}

impl BalanceProvider for DeepSeek {
    fn id(&self) -> ProviderId {
        ProviderId::new(DEEPSEEK_PROVIDER_ID)
    }

    fn display_name(&self) -> &'static str {
        "DeepSeek"
    }

    fn fetch_balance(&self) -> anyhow::Result<BalanceReport> {
        let api_key = read_api_key(API_KEY_ENV)?;
        get_json::<BalanceResponse>(&format!("{BASE_URL}/user/balance"), &api_key).map(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_balance_response_to_domain_report() {
        let response = r#"{
            "is_available": true,
            "balance_infos": [{
                "currency": "CNY",
                "total_balance": "110.00",
                "granted_balance": "10.00",
                "topped_up_balance": "100.00"
            }]
        }"#;
        let report: BalanceReport = serde_json::from_str::<BalanceResponse>(response)
            .unwrap()
            .into();

        assert!(report.is_available);
        assert_eq!(report.balance_infos[0].currency, "CNY");
        assert_eq!(report.balance_infos[0].total_balance, "110.00");
    }
}
