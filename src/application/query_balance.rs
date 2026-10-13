use anyhow::Result;
use chrono::Local;

use crate::domain::balance::{BalanceReport, ProviderId};
use crate::domain::usage_history::parse_balance;

use super::ports::{BalanceProviderRouter, UsageHistoryRepository};

pub struct BalanceQueryResult {
    pub report: BalanceReport,
    pub today_usage: f64,
    pub cache_warning: Option<anyhow::Error>,
}

pub fn execute(
    provider_id: &ProviderId,
    providers: &dyn BalanceProviderRouter,
    history_repository: &dyn UsageHistoryRepository,
) -> Result<BalanceQueryResult> {
    let provider = providers.route(provider_id)?;
    let report = provider.fetch_balance()?;
    let today = Local::now().date_naive().to_string();
    let (current_balance, currency) = report
        .balance_infos
        .first()
        .map(|info| (parse_balance(&info.total_balance), info.currency.as_str()))
        .unwrap_or((0.0, ""));
    let provider_id = provider.id();
    let history = history_repository.load();
    let today_usage = history.today_usage(&provider_id, &today, current_balance);
    let cache_warning = history_repository
        .record_query(&provider_id, &today, current_balance, currency)
        .err();

    Ok(BalanceQueryResult {
        report,
        today_usage,
        cache_warning,
    })
}
