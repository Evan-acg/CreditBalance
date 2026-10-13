//! Codex app-server JSON-RPC 适配器。

use anyhow::{Context, Result, anyhow, bail};
use serde::Deserialize;
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use crate::application::ports::CodexQuotaReader;
use crate::domain::codex_quota::{CodexUsageReport, Credits, UsageWindow};

const RPC_TIMEOUT: Duration = Duration::from_secs(30);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(1);
const SHUTDOWN_POLL_INTERVAL: Duration = Duration::from_millis(10);

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RateLimitCredits {
    pub has_credits: bool,
    pub unlimited: bool,
    pub balance: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RpcResponse {
    id: u64,
    result: Option<RateLimitsResult>,
    error: Option<RpcError>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RateLimitsResult {
    rate_limits: RateLimitSnapshot,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RateLimitSnapshot {
    plan_type: Option<String>,
    primary: Option<RateLimitWindow>,
    secondary: Option<RateLimitWindow>,
    credits: Option<RateLimitCredits>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RateLimitWindow {
    used_percent: i32,
    window_duration_mins: Option<i64>,
    resets_at: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct RpcError {
    message: String,
}

fn fetch_usage() -> Result<CodexUsageReport> {
    let mut server = AppServer::spawn()?;
    server.send(&initialize_request(env!("CARGO_PKG_VERSION")))?;
    let initialize = server.read_response(0)?;
    ensure_rpc_success(&initialize, 0)?;
    server.send(&initialized_notification())?;
    server.send(&rate_limits_request())?;
    let response = server.read_response(1)?;
    let report = parse_rate_limits_response(&response, 1)?;
    server.finish();
    Ok(report)
}

pub struct AppServerCodexQuotaReader;

impl CodexQuotaReader for AppServerCodexQuotaReader {
    fn fetch_quota(&self) -> Result<CodexUsageReport> {
        fetch_usage()
    }
}

fn parse_rate_limits_response(json: &str, expected_id: u64) -> Result<CodexUsageReport> {
    let response: RpcResponse =
        serde_json::from_str(json).context("Codex app-server 返回了无效的 JSON")?;
    if response.id != expected_id {
        bail!("Codex app-server 返回了不匹配的响应 ID");
    }
    if let Some(error) = response.error {
        return Err(rpc_error(&error.message));
    }
    let snapshot = response
        .result
        .ok_or_else(|| anyhow!("Codex app-server 响应缺少 result"))?
        .rate_limits;
    Ok(CodexUsageReport {
        plan_type: snapshot.plan_type,
        primary: snapshot.primary.map(usage_window_from_response),
        secondary: snapshot.secondary.map(usage_window_from_response),
        credits: snapshot.credits.map(|credits| Credits {
            has_credits: credits.has_credits,
            unlimited: credits.unlimited,
            balance: credits.balance,
        }),
    })
}

fn usage_window_from_response(window: RateLimitWindow) -> UsageWindow {
    UsageWindow::from_used_percent(
        window.used_percent,
        window.window_duration_mins,
        window.resets_at,
    )
}

fn initialize_request(version: &str) -> String {
    json!({
        "id": 0,
        "method": "initialize",
        "params": {
            "clientInfo": {
                "name": "credit-balance",
                "version": version,
            }
        }
    })
    .to_string()
}

fn initialized_notification() -> String {
    json!({ "method": "initialized" }).to_string()
}

fn rate_limits_request() -> String {
    json!({ "id": 1, "method": "account/rateLimits/read" }).to_string()
}

fn is_matching_response(line: &str, expected_id: u64) -> Result<bool> {
    let value: Value =
        serde_json::from_str(line).context("Codex app-server 返回了无效的 JSON 行")?;
    Ok(value.get("id").and_then(Value::as_u64) == Some(expected_id))
}

fn ensure_rpc_success(line: &str, expected_id: u64) -> Result<()> {
    let value: Value =
        serde_json::from_str(line).context("Codex app-server 返回了无效的初始化响应")?;
    if value.get("id").and_then(Value::as_u64) != Some(expected_id) {
        bail!("Codex app-server 返回了不匹配的初始化响应 ID");
    }
    if let Some(message) = value
        .get("error")
        .and_then(|error| error.get("message"))
        .and_then(Value::as_str)
    {
        return Err(rpc_error(message));
    }
    if value.get("result").is_none() {
        bail!("Codex app-server 初始化响应缺少 result");
    }
    Ok(())
}

fn rpc_error(message: &str) -> anyhow::Error {
    if message.contains("authentication required") {
        anyhow!("Codex 未通过 ChatGPT 登录，请登录 Codex Desktop 或运行 `codex login`")
    } else if message.contains("Method not found") || message.contains("method not found") {
        anyhow!("当前 Codex app-server 不支持额度查询，请升级 Codex Desktop 或 Codex CLI")
    } else {
        anyhow!("Codex app-server 请求失败")
    }
}

struct AppServer {
    child: Child,
    stdin: Option<ChildStdin>,
    output: Receiver<std::result::Result<String, String>>,
}

impl AppServer {
    fn spawn() -> Result<Self> {
        let mut child = super::runtime::spawn_app_server()?;
        let stdin = child
            .stdin
            .take()
            .context("无法连接 Codex app-server stdin")?;
        let stdout = child
            .stdout
            .take()
            .context("无法连接 Codex app-server stdout")?;
        let (sender, output) = mpsc::channel();
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let message = line.map_err(|error| error.to_string());
                if sender.send(message).is_err() {
                    break;
                }
            }
        });
        Ok(Self {
            child,
            stdin: Some(stdin),
            output,
        })
    }

    fn send(&mut self, message: &str) -> Result<()> {
        let stdin = self
            .stdin
            .as_mut()
            .context("Codex app-server stdin 已关闭")?;
        writeln!(stdin, "{message}").context("向 Codex app-server 发送请求失败")?;
        stdin.flush().context("刷新 Codex app-server 请求失败")
    }

    fn read_response(&self, expected_id: u64) -> Result<String> {
        let deadline = Instant::now() + RPC_TIMEOUT;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                bail!("等待 Codex app-server 响应超时");
            }
            match self.output.recv_timeout(remaining) {
                Ok(Ok(line)) if is_matching_response(&line, expected_id)? => return Ok(line),
                Ok(Ok(_)) => continue,
                Ok(Err(error)) => return Err(anyhow!("读取 Codex app-server 输出失败: {error}")),
                Err(RecvTimeoutError::Timeout) => bail!("等待 Codex app-server 响应超时"),
                Err(RecvTimeoutError::Disconnected) => {
                    bail!("Codex app-server 在返回响应前已退出")
                }
            }
        }
    }

    fn finish(&mut self) {
        self.stdin.take();
        let deadline = Instant::now() + SHUTDOWN_TIMEOUT;
        while Instant::now() < deadline {
            if self.child.try_wait().ok().flatten().is_some() {
                return;
            }
            thread::sleep(SHUTDOWN_POLL_INTERVAL);
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for AppServer {
    fn drop(&mut self) {
        self.finish();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_primary_and_secondary_windows_to_remaining_quota() {
        let json = r#"{
            "id": 1,
            "result": {
                "rateLimits": {
                    "planType": "plus",
                    "primary": {
                        "usedPercent": 42,
                        "windowDurationMins": 300,
                        "resetsAt": 1787472000
                    },
                    "secondary": {
                        "usedPercent": 84,
                        "windowDurationMins": 10080,
                        "resetsAt": 1788076800
                    },
                    "credits": null
                }
            }
        }"#;

        let report = parse_rate_limits_response(json, 1).unwrap();

        assert_eq!(report.plan_type.as_deref(), Some("plus"));
        let primary = report.primary.unwrap();
        assert_eq!(primary.remaining_percent, 58);
        assert_eq!(primary.window_duration_mins, Some(300));
        assert_eq!(primary.resets_at, Some(1787472000));
        let secondary = report.secondary.unwrap();
        assert_eq!(secondary.remaining_percent, 16);
        assert_eq!(secondary.window_duration_mins, Some(10080));
    }

    #[test]
    fn builds_official_app_server_request_sequence() {
        assert_eq!(
            initialize_request("0.1.0"),
            r#"{"id":0,"method":"initialize","params":{"clientInfo":{"name":"credit-balance","version":"0.1.0"}}}"#
        );
        assert_eq!(initialized_notification(), r#"{"method":"initialized"}"#);
        assert_eq!(
            rate_limits_request(),
            r#"{"id":1,"method":"account/rateLimits/read"}"#
        );
    }

    #[test]
    fn identifies_only_the_response_with_the_expected_id() {
        assert!(!is_matching_response(r#"{"method":"account/updated"}"#, 1).unwrap());
        assert!(!is_matching_response(r#"{"id":0,"result":{}}"#, 1).unwrap());
        assert!(is_matching_response(r#"{"id":1,"result":{}}"#, 1).unwrap());
    }

    #[test]
    fn does_not_echo_unknown_app_server_error_details() {
        let error = rpc_error("upstream failed with access_token=secret");

        let message = error.to_string();
        assert_eq!(message, "Codex app-server 请求失败");
        assert!(!message.contains("secret"));
    }
}
