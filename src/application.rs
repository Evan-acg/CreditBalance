use anyhow::{Result, anyhow};

use crate::cache;
use crate::cli::{Command, OutputFormat};
use crate::codex_usage;
use crate::output::{render_balance_text, write_rendered};
use crate::providers::Provider;
use crate::providers::deepseek::DeepSeek;
use crate::providers::openrouter::OpenRouter;

pub(crate) fn run(command: Command) -> Result<()> {
    match command {
        Command::Gpt { format, output } => run_gpt(format, output),
        balance => run_balance(balance),
    }
}

fn run_gpt(format: OutputFormat, output: Option<String>) -> Result<()> {
    let report = codex_usage::fetch_usage()?;
    let rendered = match format {
        OutputFormat::Text => codex_usage::render_text(&report)?,
        OutputFormat::Json => codex_usage::render_json(&report)?,
    };
    write_rendered(&rendered, output)
}

fn run_balance(command: Command) -> Result<()> {
    let (provider, format, output): (Box<dyn Provider>, OutputFormat, Option<String>) =
        match command {
            Command::Ds { format, output } => (Box::new(DeepSeek), format, output),
            Command::Or { format, output } => (Box::new(OpenRouter), format, output),
            Command::Gpt { .. } => unreachable!("GPT command is dispatched before balance"),
        };
    let api_key = read_api_key(provider.as_ref())?;
    let report = provider.fetch_balance(&api_key)?;
    let usage = calculate_and_record_usage(provider.as_ref(), &report);
    let rendered = match format {
        OutputFormat::Text => render_balance_text(&report, usage),
        OutputFormat::Json => serde_json::to_string_pretty(&serde_json::json!({
            "balance": report,
            "today_usage": format!("{usage:.2}"),
        }))?,
    };
    write_rendered(&rendered, output)
}

fn read_api_key(provider: &dyn Provider) -> Result<String> {
    std::env::var(provider.api_key_env_name()).map_err(|_| {
        anyhow!(
            "环境变量 {} 未设置，请先导出，例如: $env:{} = \"sk-...\"",
            provider.api_key_env_name(),
            provider.api_key_env_name()
        )
    })
}

fn calculate_and_record_usage(
    provider: &dyn Provider,
    report: &crate::providers::BalanceReport,
) -> f64 {
    let today = cache::today_str();
    let (current, currency) = report
        .balance_infos
        .first()
        .map(|info| {
            (
                cache::parse_balance(&info.total_balance),
                info.currency.clone(),
            )
        })
        .unwrap_or((0.0, String::new()));
    let Some(path) = cache::cache_path() else {
        return 0.0;
    };
    let usage = cache::today_usage(&cache::load(&path).records, provider.id(), &today, current);
    if let Err(error) = cache::record_query(&path, provider.id(), &today, current, &currency) {
        eprintln!("警告: 写入缓存失败: {error:#}");
    }
    usage
}
