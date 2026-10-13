//! CLI 输出格式化及 stdout/文件呈现。

use std::fs::File;
use std::io::Write;

use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, Local, Utc};
use serde_json::{Value, json};

use crate::domain::balance::BalanceReport;
use crate::domain::codex_quota::{CodexUsageReport, UsageWindow};

pub(crate) fn write_rendered(rendered: &str, output: Option<String>) -> Result<()> {
    match output {
        Some(path) => {
            let mut file = File::create(&path).with_context(|| format!("无法创建文件 {path}"))?;
            writeln!(file, "{rendered}").with_context(|| format!("写入文件 {path} 失败"))?;
        }
        None => println!("{rendered}"),
    }
    Ok(())
}

pub(crate) fn render_balance_text(report: &BalanceReport, usage: f64) -> String {
    let mut output = String::new();
    output.push_str(if report.is_available {
        "状态: 可用\n"
    } else {
        "状态: 不可用\n"
    });
    for info in &report.balance_infos {
        output.push_str(&format!(
            "总余额 {} {}（充值 {} / 赠送 {}）\n",
            info.total_balance, info.currency, info.topped_up_balance, info.granted_balance
        ));
    }
    if let Some(info) = report.balance_infos.first() {
        output.push_str(&format!("今日消耗 {usage:.2} {}\n", info.currency));
    }
    output
}

pub(crate) fn render_balance_json(report: &BalanceReport, usage: f64) -> Result<String> {
    let value = json!({
        "balance": {
            "is_available": report.is_available,
            "balance_infos": report.balance_infos.iter().map(|info| json!({
                "currency": info.currency,
                "total_balance": info.total_balance,
                "granted_balance": info.granted_balance,
                "topped_up_balance": info.topped_up_balance,
            })).collect::<Vec<_>>(),
        },
        "today_usage": format!("{usage:.2}"),
    });
    serde_json::to_string_pretty(&value).context("序列化余额结果失败")
}

pub(crate) fn render_codex_json(report: &CodexUsageReport) -> Result<String> {
    let value = json!({
        "provider": "gpt",
        "plan_type": report.plan_type,
        "primary": window_json(report.primary.as_ref())?,
        "secondary": window_json(report.secondary.as_ref())?,
        "credits": report.credits.as_ref().map(|credits| json!({
            "hasCredits": credits.has_credits,
            "unlimited": credits.unlimited,
            "balance": credits.balance,
        })),
    });
    serde_json::to_string_pretty(&value).context("序列化 Codex 额度失败")
}

pub(crate) fn render_codex_text(report: &CodexUsageReport) -> Result<String> {
    let mut output = String::new();
    if let Some(plan_type) = &report.plan_type {
        output.push_str(&format!("订阅计划: {plan_type}\n"));
    }
    append_window(&mut output, "主窗口", report.primary.as_ref())?;
    append_window(&mut output, "次窗口", report.secondary.as_ref())?;
    if report.primary.is_none() && report.secondary.is_none() {
        output.push_str("未返回额度窗口\n");
    }
    if let Some(credits) = &report.credits {
        if credits.unlimited {
            output.push_str("额外 Credits: 不限量\n");
        } else if credits.has_credits
            && let Some(balance) = &credits.balance
        {
            output.push_str(&format!("额外 Credits: {balance}\n"));
        }
    }
    Ok(output)
}

fn append_window(
    output: &mut String,
    fallback_name: &str,
    window: Option<&UsageWindow>,
) -> Result<()> {
    let Some(window) = window else {
        return Ok(());
    };
    let name = window
        .window_duration_mins
        .map(format_window_duration)
        .unwrap_or_else(|| fallback_name.to_string());
    output.push_str(&format!(
        "{name}: 剩余 {}%（已用 {}%）\n",
        window.remaining_percent, window.used_percent
    ));
    if let Some(timestamp) = window.resets_at {
        let reset = DateTime::<Utc>::from_timestamp(timestamp, 0)
            .ok_or_else(|| anyhow!("Codex 返回了无效的重置时间: {timestamp}"))?
            .with_timezone(&Local);
        output.push_str(&format!(
            "重置时间: {}\n",
            reset.format("%Y-%m-%d %H:%M:%S %:z")
        ));
    }
    Ok(())
}

fn format_window_duration(minutes: i64) -> String {
    if minutes % (24 * 60) == 0 {
        format!("{} 天额度", minutes / (24 * 60))
    } else if minutes % 60 == 0 {
        format!("{} 小时额度", minutes / 60)
    } else {
        format!("{minutes} 分钟额度")
    }
}

fn window_json(window: Option<&UsageWindow>) -> Result<Value> {
    let Some(window) = window else {
        return Ok(Value::Null);
    };
    let resets_at = window.resets_at.map(format_utc_timestamp).transpose()?;
    Ok(json!({
        "used_percent": window.used_percent,
        "remaining_percent": window.remaining_percent,
        "window_duration_mins": window.window_duration_mins,
        "resets_at": resets_at,
    }))
}

fn format_utc_timestamp(timestamp: i64) -> Result<String> {
    DateTime::<Utc>::from_timestamp(timestamp, 0)
        .map(|time| time.to_rfc3339())
        .ok_or_else(|| anyhow!("Codex 返回了无效的重置时间: {timestamp}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::balance::BalanceInfo;
    use crate::domain::codex_quota::Credits;

    #[test]
    fn renders_balance_json_with_the_existing_schema() {
        // Given a balance report and today's calculated usage
        let report = BalanceReport {
            is_available: true,
            balance_infos: vec![BalanceInfo {
                currency: "CNY".to_string(),
                total_balance: "110.00".to_string(),
                granted_balance: "10.00".to_string(),
                topped_up_balance: "100.00".to_string(),
            }],
        };

        // When the CLI presenter renders JSON
        let rendered = render_balance_json(&report, 12.34).unwrap();
        let value: Value = serde_json::from_str(&rendered).unwrap();

        // Then the existing balance and usage fields remain unchanged
        assert_eq!(value["balance"]["is_available"], true);
        assert_eq!(value["balance"]["balance_infos"][0]["currency"], "CNY");
        assert_eq!(
            value["balance"]["balance_infos"][0]["total_balance"],
            "110.00"
        );
        assert_eq!(value["today_usage"], "12.34");
    }

    #[test]
    fn renders_stable_json_with_rfc3339_reset_time() {
        let report = CodexUsageReport {
            plan_type: Some("plus".to_string()),
            primary: Some(UsageWindow::from_used_percent(42, Some(300), Some(0))),
            secondary: None,
            credits: None,
        };

        let rendered = render_codex_json(&report).unwrap();
        let value: Value = serde_json::from_str(&rendered).unwrap();

        assert_eq!(value["provider"], "gpt");
        assert_eq!(value["plan_type"], "plus");
        assert_eq!(value["primary"]["remaining_percent"], 58);
        assert_eq!(value["primary"]["resets_at"], "1970-01-01T00:00:00+00:00");
    }

    #[test]
    fn renders_both_subscription_windows_as_text() {
        let report = CodexUsageReport {
            plan_type: Some("plus".to_string()),
            primary: Some(UsageWindow::from_used_percent(42, Some(300), None)),
            secondary: Some(UsageWindow::from_used_percent(84, Some(10080), None)),
            credits: None,
        };

        assert_eq!(
            render_codex_text(&report).unwrap(),
            "订阅计划: plus\n5 小时额度: 剩余 58%（已用 42%）\n7 天额度: 剩余 16%（已用 84%）\n"
        );
    }

    #[test]
    fn hides_unavailable_extra_credits_from_text() {
        let report = CodexUsageReport {
            plan_type: Some("plus".to_string()),
            primary: None,
            secondary: None,
            credits: Some(Credits {
                has_credits: false,
                unlimited: false,
                balance: Some("0".to_string()),
            }),
        };

        assert!(!render_codex_text(&report).unwrap().contains("额外 Credits"));
    }
}
