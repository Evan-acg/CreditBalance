mod parser;
mod presenter;

use anyhow::Result;

use crate::application::Application;
use crate::domain::balance::ProviderId;

use self::presenter::{
    render_balance_json, render_balance_text, render_codex_json, render_codex_text, write_rendered,
};

pub(crate) use parser::{Action, Command, OutputFormat, parse_args};

pub(crate) fn execute(command: Command, application: &Application) -> Result<()> {
    match command {
        Command::Balance {
            provider_id,
            format,
            output,
        } => run_balance(provider_id, format, output, application),
        Command::Gpt { format, output } => {
            let report = application.query_codex_quota()?;
            let rendered = match format {
                OutputFormat::Text => render_codex_text(&report)?,
                OutputFormat::Json => render_codex_json(&report)?,
            };
            write_rendered(&rendered, output)
        }
    }
}

fn run_balance(
    provider_id: ProviderId,
    format: OutputFormat,
    output: Option<String>,
    application: &Application,
) -> Result<()> {
    let result = application.query_balance(&provider_id)?;
    if let Some(warning) = result.cache_warning {
        eprintln!("警告: 写入缓存失败: {warning:#}");
    }
    let rendered = match format {
        OutputFormat::Text => render_balance_text(&result.report, result.today_usage),
        OutputFormat::Json => render_balance_json(&result.report, result.today_usage)?,
    };
    write_rendered(&rendered, output)
}
