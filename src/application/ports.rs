use anyhow::Result;

use crate::domain::balance::{BalanceReport, ProviderId};
use crate::domain::codex_quota::CodexUsageReport;
use crate::domain::usage_history::UsageHistory;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BalanceProviderDescriptor {
    pub id: ProviderId,
    pub display_name: &'static str,
}

pub trait BalanceProvider {
    fn id(&self) -> ProviderId;
    fn display_name(&self) -> &'static str;
    fn fetch_balance(&self) -> Result<BalanceReport>;
}

pub trait BalanceProviderRouter {
    fn route(&self, id: &ProviderId) -> Result<&dyn BalanceProvider>;
}

pub trait CodexQuotaReader {
    fn fetch_quota(&self) -> Result<CodexUsageReport>;
}

pub trait UsageHistoryRepository {
    fn load(&self) -> UsageHistory;
    fn record_query(
        &self,
        provider: &ProviderId,
        today: &str,
        balance: f64,
        currency: &str,
    ) -> Result<()>;
}
