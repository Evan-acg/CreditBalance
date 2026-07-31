use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Context;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheRecord {
    pub provider: String,
    pub day: String,
    pub balance: f64,
    pub currency: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CacheFile {
    pub records: Vec<CacheRecord>,
}

pub fn today_str() -> String {
    chrono::Local::now().date_naive().to_string()
}

pub fn cache_path() -> Option<PathBuf> {
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

pub fn load(path: &Path) -> CacheFile {
    match fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
        Err(_) => CacheFile::default(),
    }
}

pub fn parse_balance(s: &str) -> f64 {
    s.parse().unwrap_or(0.0)
}

/// 今日消耗 = 今天最早一次查询余额 - 当前余额；今天无记录则为 0
pub fn today_usage(records: &[CacheRecord], provider: &str, today: &str, current: f64) -> f64 {
    match records
        .iter()
        .find(|r| r.provider == provider && r.day == today)
    {
        Some(baseline) => baseline.balance - current,
        None => 0.0,
    }
}

/// 记录本次查询并裁剪非今日记录，返回序列化结果用于写入
pub fn record_query(
    path: &Path,
    provider: &str,
    today: &str,
    balance: f64,
    currency: &str,
) -> anyhow::Result<()> {
    let mut cache = load(path);
    cache.records.retain(|r| r.day == today);
    cache.records.push(CacheRecord {
        provider: provider.to_string(),
        day: today.to_string(),
        balance,
        currency: currency.to_string(),
    });
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| "无法创建缓存目录")?;
    }
    let text = serde_json::to_string_pretty(&cache).context("序列化缓存失败")?;
    fs::write(path, text).with_context(|| "无法写入缓存文件")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(provider: &str, day: &str, balance: f64) -> CacheRecord {
        CacheRecord {
            provider: provider.to_string(),
            day: day.to_string(),
            balance,
            currency: "CNY".to_string(),
        }
    }

    #[test]
    fn usage_without_records_is_zero() {
        assert_eq!(today_usage(&[], "ds", "2026-07-31", 100.0), 0.0);
    }

    #[test]
    fn usage_uses_earliest_balance_of_today() {
        let records = vec![
            record("ds", "2026-07-31", 120.0),
            record("ds", "2026-07-31", 110.0),
        ];
        assert_eq!(today_usage(&records, "ds", "2026-07-31", 100.0), 20.0);
    }

    #[test]
    fn usage_ignores_other_days_and_providers() {
        let records = vec![
            record("ds", "2026-07-30", 500.0),
            record("or", "2026-07-31", 999.0),
        ];
        assert_eq!(today_usage(&records, "ds", "2026-07-31", 100.0), 0.0);
    }

    #[test]
    fn parse_balance_parses_string() {
        assert_eq!(parse_balance("110.00"), 110.0);
        assert_eq!(parse_balance("74.75"), 74.75);
        assert_eq!(parse_balance("oops"), 0.0);
    }

    #[test]
    fn record_query_appends_and_prunes() {
        let dir = env::temp_dir().join(format!("crd-test-{}", std::process::id()));
        let path = dir.join("cache.json");
        let _ = fs::remove_file(&path);

        record_query(&path, "ds", "2026-07-31", 110.0, "CNY").unwrap();
        record_query(&path, "ds", "2026-07-31", 100.0, "CNY").unwrap();

        let cache = load(&path);
        assert_eq!(cache.records.len(), 2);
        assert_eq!(cache.records[0].balance, 110.0);
        assert_eq!(cache.records[1].balance, 100.0);

        record_query(&path, "ds", "2026-07-30", 1.0, "CNY").unwrap();

        let cache = load(&path);
        assert_eq!(cache.records.len(), 1);
        assert_eq!(cache.records[0].day, "2026-07-30");
        assert_eq!(cache.records[0].balance, 1.0);

        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir(&dir);
    }
}
