use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, Eq, Hash, PartialEq)]
pub struct ProviderId(String);

impl ProviderId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for ProviderId {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

pub const DEEPSEEK_PROVIDER_ID: &str = "ds";
pub const OPENROUTER_PROVIDER_ID: &str = "or";

#[derive(Debug, Clone, PartialEq)]
pub struct BalanceInfo {
    pub currency: String,
    pub total_balance: String,
    pub granted_balance: String,
    pub topped_up_balance: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BalanceReport {
    pub is_available: bool,
    pub balance_infos: Vec<BalanceInfo>,
}
