use anyhow::{Context, Result, anyhow, bail};
use chrono::{DateTime, Local, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

const RPC_TIMEOUT: Duration = Duration::from_secs(30);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(1);
const SHUTDOWN_POLL_INTERVAL: Duration = Duration::from_millis(10);

#[derive(Debug, Serialize, PartialEq)]
pub struct CodexUsageReport {
    pub plan_type: Option<String>,
    pub primary: Option<UsageWindow>,
    pub secondary: Option<UsageWindow>,
    pub credits: Option<Credits>,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct UsageWindow {
    pub used_percent: i32,
    pub remaining_percent: i32,
    pub window_duration_mins: Option<i64>,
    pub resets_at: Option<i64>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Credits {
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
    credits: Option<Credits>,
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

pub fn fetch_usage() -> Result<CodexUsageReport> {
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

pub fn parse_rate_limits_response(json: &str, expected_id: u64) -> Result<CodexUsageReport> {
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
        primary: snapshot.primary.map(UsageWindow::from),
        secondary: snapshot.secondary.map(UsageWindow::from),
        credits: snapshot.credits,
    })
}

impl From<RateLimitWindow> for UsageWindow {
    fn from(window: RateLimitWindow) -> Self {
        Self {
            used_percent: window.used_percent,
            remaining_percent: 100_i32.saturating_sub(window.used_percent).clamp(0, 100),
            window_duration_mins: window.window_duration_mins,
            resets_at: window.resets_at,
        }
    }
}

pub fn render_json(report: &CodexUsageReport) -> Result<String> {
    let value = json!({
        "provider": "gpt",
        "plan_type": report.plan_type,
        "primary": window_json(report.primary.as_ref())?,
        "secondary": window_json(report.secondary.as_ref())?,
        "credits": report.credits,
    });
    serde_json::to_string_pretty(&value).context("序列化 Codex 额度失败")
}

pub fn render_text(report: &CodexUsageReport) -> Result<String> {
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
        anyhow!("Codex CLI 未通过 ChatGPT 登录，请先运行 `codex login`")
    } else if message.contains("Method not found") || message.contains("method not found") {
        anyhow!("当前 Codex CLI 版本不支持额度查询，请升级 Codex CLI")
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
        let mut child = spawn_codex()?;
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

fn spawn_codex() -> Result<Child> {
    let mut command = Command::new("codex");
    configure_command(&mut command);
    match command.spawn() {
        Ok(child) => Ok(child),
        #[cfg(windows)]
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let mut command = Command::new("codex.cmd");
            configure_command(&mut command);
            command.spawn().map_err(spawn_error)
        }
        Err(error) => Err(spawn_error(error)),
    }
}

fn configure_command(command: &mut Command) {
    command
        .arg("app-server")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
}

fn spawn_error(error: std::io::Error) -> anyhow::Error {
    if error.kind() == std::io::ErrorKind::NotFound {
        anyhow!("找不到 Codex CLI，请先安装并运行 `codex login`")
    } else {
        anyhow!(error).context("无法启动 Codex app-server")
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
    fn renders_stable_json_with_rfc3339_reset_time() {
        let report = CodexUsageReport {
            plan_type: Some("plus".to_string()),
            primary: Some(UsageWindow {
                used_percent: 42,
                remaining_percent: 58,
                window_duration_mins: Some(300),
                resets_at: Some(0),
            }),
            secondary: None,
            credits: None,
        };

        let rendered = render_json(&report).unwrap();
        let value: serde_json::Value = serde_json::from_str(&rendered).unwrap();

        assert_eq!(value["provider"], "gpt");
        assert_eq!(value["plan_type"], "plus");
        assert_eq!(value["primary"]["remaining_percent"], 58);
        assert_eq!(value["primary"]["resets_at"], "1970-01-01T00:00:00+00:00");
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
    fn renders_both_subscription_windows_as_text() {
        let report = CodexUsageReport {
            plan_type: Some("plus".to_string()),
            primary: Some(UsageWindow {
                used_percent: 42,
                remaining_percent: 58,
                window_duration_mins: Some(300),
                resets_at: None,
            }),
            secondary: Some(UsageWindow {
                used_percent: 84,
                remaining_percent: 16,
                window_duration_mins: Some(10080),
                resets_at: None,
            }),
            credits: None,
        };

        assert_eq!(
            render_text(&report).unwrap(),
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

        let rendered = render_text(&report).unwrap();

        assert!(!rendered.contains("额外 Credits"));
    }

    #[test]
    fn does_not_echo_unknown_app_server_error_details() {
        let error = rpc_error("upstream failed with access_token=secret");

        let message = error.to_string();
        assert_eq!(message, "Codex app-server 请求失败");
        assert!(!message.contains("secret"));
    }
}
