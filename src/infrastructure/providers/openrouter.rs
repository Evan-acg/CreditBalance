use serde::Deserialize;

use super::{http::get_json, read_api_key};
use crate::application::ports::BalanceProvider;
use crate::domain::balance::{BalanceInfo, BalanceReport, OPENROUTER_PROVIDER_ID, ProviderId};

pub struct OpenRouter;

const API_KEY_ENV: &str = "OR_BALANCE_API_KEY";
const CREDITS_URL: &str = "https://openrouter.ai/api/v1/credits";

#[derive(Debug, Deserialize)]
struct CreditsResponse {
    data: CreditsData,
}

#[derive(Debug, Deserialize)]
struct CreditsData {
    total_credits: f64,
    total_usage: f64,
}

impl BalanceProvider for OpenRouter {
    fn id(&self) -> ProviderId {
        ProviderId::new(OPENROUTER_PROVIDER_ID)
    }

    fn display_name(&self) -> &'static str {
        "OpenRouter"
    }

    fn fetch_balance(&self) -> anyhow::Result<BalanceReport> {
        let api_key = read_api_key(API_KEY_ENV)?;
        let resp: CreditsResponse = get_json(CREDITS_URL, &api_key)?;
        Ok(map_to_report(
            resp.data.total_credits,
            resp.data.total_usage,
        ))
    }
}

fn map_to_report(total: f64, used: f64) -> BalanceReport {
    let remaining = total - used;
    BalanceReport {
        is_available: remaining > 0.0,
        balance_infos: vec![BalanceInfo {
            currency: "USD".to_string(),
            total_balance: format!("{remaining:.2}"),
            granted_balance: "0.00".to_string(),
            topped_up_balance: format!("{total:.2}"),
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RESPONSE: &str = r#"{"data": {"total_credits": 100.5, "total_usage": 25.75}}"#;

    #[test]
    fn parses_credits_response() {
        let resp: CreditsResponse = serde_json::from_str(RESPONSE).unwrap();
        assert_eq!(resp.data.total_credits, 100.5);
        assert_eq!(resp.data.total_usage, 25.75);
    }

    #[test]
    fn maps_to_balance_report() {
        let report = map_to_report(100.5, 25.75);
        assert!(report.is_available);
        assert_eq!(report.balance_infos.len(), 1);
        let info = &report.balance_infos[0];
        assert_eq!(info.currency, "USD");
        assert_eq!(info.total_balance, "74.75");
        assert_eq!(info.topped_up_balance, "100.50");
        assert_eq!(info.granted_balance, "0.00");
    }
}
