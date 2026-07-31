use super::{BalanceReport, Provider};

pub struct DeepSeek;

const API_KEY_ENV: &str = "DS_BALANCE_API_KEY";
const BASE_URL: &str = "https://api.deepseek.com";

impl Provider for DeepSeek {
    fn id(&self) -> &'static str {
        "ds"
    }

    fn api_key_env_name(&self) -> &'static str {
        API_KEY_ENV
    }

    fn fetch_balance(&self, api_key: &str) -> Result<BalanceReport, anyhow::Error> {
        super::get_json(&format!("{BASE_URL}/user/balance"), api_key)
    }
}
