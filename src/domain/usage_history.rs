use crate::domain::balance::ProviderId;

#[derive(Debug, Clone, PartialEq)]
pub struct BalanceSnapshot {
    pub provider: String,
    pub day: String,
    pub balance: f64,
    pub currency: String,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct UsageHistory {
    records: Vec<BalanceSnapshot>,
}

impl UsageHistory {
    pub fn from_records(records: Vec<BalanceSnapshot>) -> Self {
        Self { records }
    }

    pub fn records(&self) -> &[BalanceSnapshot] {
        &self.records
    }

    pub fn today_usage(&self, provider: &ProviderId, today: &str, current: f64) -> f64 {
        self.records
            .iter()
            .find(|record| record.provider == provider.as_str() && record.day == today)
            .map(|baseline| baseline.balance - current)
            .unwrap_or(0.0)
    }

    pub fn record_query(
        &mut self,
        provider: &ProviderId,
        today: &str,
        balance: f64,
        currency: &str,
    ) {
        self.records.retain(|record| record.day == today);
        self.records.push(BalanceSnapshot {
            provider: provider.to_string(),
            day: today.to_string(),
            balance,
            currency: currency.to_string(),
        });
    }
}

pub fn parse_balance(value: &str) -> f64 {
    value.parse().unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(provider: &str, day: &str, balance: f64) -> BalanceSnapshot {
        BalanceSnapshot {
            provider: provider.to_string(),
            day: day.to_string(),
            balance,
            currency: "CNY".to_string(),
        }
    }

    #[test]
    fn usage_without_records_is_zero() {
        let history = UsageHistory::default();
        assert_eq!(
            history.today_usage(&ProviderId::new("ds"), "2026-07-31", 100.0),
            0.0
        );
    }

    #[test]
    fn usage_uses_earliest_balance_of_today() {
        let history = UsageHistory::from_records(vec![
            record("ds", "2026-07-31", 120.0),
            record("ds", "2026-07-31", 110.0),
        ]);
        assert_eq!(
            history.today_usage(&ProviderId::new("ds"), "2026-07-31", 100.0),
            20.0
        );
    }

    #[test]
    fn usage_ignores_other_days_and_providers() {
        let history = UsageHistory::from_records(vec![
            record("ds", "2026-07-30", 500.0),
            record("or", "2026-07-31", 999.0),
        ]);
        assert_eq!(
            history.today_usage(&ProviderId::new("ds"), "2026-07-31", 100.0),
            0.0
        );
    }

    #[test]
    fn parse_balance_parses_string() {
        assert_eq!(parse_balance("110.00"), 110.0);
        assert_eq!(parse_balance("74.75"), 74.75);
        assert_eq!(parse_balance("oops"), 0.0);
    }

    #[test]
    fn recording_query_prunes_old_days_and_preserves_same_day_history() {
        let mut history = UsageHistory::from_records(vec![
            record("ds", "2026-07-30", 500.0),
            record("ds", "2026-07-31", 110.0),
        ]);

        history.record_query(&ProviderId::new("ds"), "2026-07-31", 100.0, "CNY");

        assert_eq!(history.records().len(), 2);
        assert!(
            history
                .records()
                .iter()
                .all(|record| record.day == "2026-07-31")
        );
        assert_eq!(history.records()[1].balance, 100.0);
    }
}
