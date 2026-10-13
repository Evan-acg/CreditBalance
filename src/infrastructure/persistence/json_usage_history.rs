use std::env;
use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::application::ports::UsageHistoryRepository;
use crate::domain::balance::ProviderId;
use crate::domain::usage_history::{BalanceSnapshot, UsageHistory};

pub struct JsonUsageHistoryRepository {
    path: Option<PathBuf>,
}

impl Default for JsonUsageHistoryRepository {
    fn default() -> Self {
        Self { path: cache_path() }
    }
}

impl UsageHistoryRepository for JsonUsageHistoryRepository {
    fn load(&self) -> UsageHistory {
        let Some(path) = &self.path else {
            return UsageHistory::default();
        };
        let Ok(text) = fs::read_to_string(path) else {
            return UsageHistory::default();
        };
        serde_json::from_str::<PersistedUsageHistory>(&text)
            .map(|persisted| {
                UsageHistory::from_records(
                    persisted
                        .records
                        .into_iter()
                        .map(BalanceSnapshot::from)
                        .collect(),
                )
            })
            .unwrap_or_default()
    }

    fn record_query(
        &self,
        provider: &ProviderId,
        today: &str,
        balance: f64,
        currency: &str,
    ) -> Result<()> {
        if self.path.is_none() {
            return Ok(());
        }
        let mut history = self.load();
        history.record_query(provider, today, balance, currency);
        self.save_history(&history)
    }
}

impl JsonUsageHistoryRepository {
    fn save_history(&self, history: &UsageHistory) -> Result<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| "无法创建缓存目录")?;
        }
        let persisted = PersistedUsageHistory {
            records: history.records().iter().map(Into::into).collect(),
        };
        let text = serde_json::to_string_pretty(&persisted).context("序列化缓存失败")?;
        fs::write(path, text).with_context(|| "无法写入缓存文件")
    }
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct PersistedUsageHistory {
    records: Vec<PersistedBalanceSnapshot>,
}

#[derive(Debug, Deserialize, Serialize)]
struct PersistedBalanceSnapshot {
    provider: String,
    day: String,
    balance: f64,
    currency: String,
}

impl From<PersistedBalanceSnapshot> for BalanceSnapshot {
    fn from(snapshot: PersistedBalanceSnapshot) -> Self {
        Self {
            provider: snapshot.provider,
            day: snapshot.day,
            balance: snapshot.balance,
            currency: snapshot.currency,
        }
    }
}

impl From<&BalanceSnapshot> for PersistedBalanceSnapshot {
    fn from(snapshot: &BalanceSnapshot) -> Self {
        Self {
            provider: snapshot.provider.clone(),
            day: snapshot.day.clone(),
            balance: snapshot.balance,
            currency: snapshot.currency.clone(),
        }
    }
}

fn cache_path() -> Option<PathBuf> {
    if let Ok(appdata) = env::var("APPDATA")
        && !appdata.is_empty()
    {
        return Some(PathBuf::from(appdata).join("crd").join("cache.json"));
    }
    if let Ok(xdg) = env::var("XDG_CACHE_HOME")
        && !xdg.is_empty()
    {
        return Some(PathBuf::from(xdg).join("crd").join("cache.json"));
    }
    for home in ["HOME", "USERPROFILE"] {
        if let Ok(home) = env::var(home)
            && !home.is_empty()
        {
            return Some(
                PathBuf::from(home)
                    .join(".cache")
                    .join("crd")
                    .join("cache.json"),
            );
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repository_records_queries_and_preserves_json_schema() {
        let dir = env::temp_dir().join(format!("crd-test-{}", std::process::id()));
        let path = dir.join("cache.json");
        let repository = JsonUsageHistoryRepository {
            path: Some(path.clone()),
        };
        repository
            .record_query(&ProviderId::new("ds"), "2026-07-30", 500.0, "CNY")
            .unwrap();
        repository
            .record_query(&ProviderId::new("ds"), "2026-07-31", 110.0, "CNY")
            .unwrap();
        repository
            .record_query(&ProviderId::new("ds"), "2026-07-31", 100.0, "CNY")
            .unwrap();

        let loaded = repository.load();
        assert_eq!(loaded.records().len(), 2);
        assert_eq!(loaded.records()[0].balance, 110.0);
        assert_eq!(loaded.records()[1].balance, 100.0);
        let json = fs::read_to_string(&path).unwrap();
        assert!(json.contains("\"provider\": \"ds\""));
        assert!(json.contains("\"day\": \"2026-07-31\""));

        fs::remove_dir_all(dir).unwrap();
    }
}
