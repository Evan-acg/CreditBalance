pub mod ports;
pub mod provider_registry;
pub mod query_balance;
pub mod query_codex_quota;

use anyhow::Result;

use crate::domain::balance::ProviderId;
use crate::domain::codex_quota::CodexUsageReport;

use self::ports::{CodexQuotaReader, UsageHistoryRepository};
use self::provider_registry::ProviderRegistry;
use self::query_balance::BalanceQueryResult;

pub struct Application {
    providers: ProviderRegistry,
    usage_history: Box<dyn UsageHistoryRepository>,
    codex_quota_reader: Box<dyn CodexQuotaReader>,
}

impl Application {
    pub fn new(
        providers: ProviderRegistry,
        usage_history: Box<dyn UsageHistoryRepository>,
        codex_quota_reader: Box<dyn CodexQuotaReader>,
    ) -> Self {
        Self {
            providers,
            usage_history,
            codex_quota_reader,
        }
    }

    pub fn query_balance(&self, provider: &ProviderId) -> Result<BalanceQueryResult> {
        query_balance::execute(provider, &self.providers, self.usage_history.as_ref())
    }

    pub fn query_codex_quota(&self) -> Result<CodexUsageReport> {
        query_codex_quota::execute(self.codex_quota_reader.as_ref())
    }

    pub fn balance_provider_descriptors(&self) -> Vec<ports::BalanceProviderDescriptor> {
        self.providers.descriptors()
    }
}
