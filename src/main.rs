mod cache;
mod providers;

use std::fs::File;
use std::io::Write;
use std::process::ExitCode;

use anyhow::{anyhow, Context, Result};
use providers::deepseek::DeepSeek;
use providers::openrouter::OpenRouter;
use providers::{BalanceReport, Provider};

const USAGE: &str = "查询 AI 平台账户余额

用法: crd <COMMAND> [选项]

命令:
  ds    查询 DeepSeek 余额
  or    查询 OpenRouter 余额
  help  Print this message or the help of the given subcommand(s)

选项:
  -h, --help     Print help
  -V, --version  Print version";

const DS_USAGE: &str = "用法: crd ds [--format <text|json>] [-o <文件>] [-t <单位>]

选项:
      --format <text|json>  输出格式（默认: text）
  -o, --output <文件>       将输出写入文件
  -t, --today <单位>        今日消耗时间单位（默认: d=天，默认显示）
  -h, --help                Print help";

const OR_USAGE: &str = "用法: crd or [--format <text|json>] [-o <文件>] [-t <单位>]

选项:
      --format <text|json>  输出格式（默认: text）
  -o, --output <文件>       将输出写入文件
  -t, --today <单位>        今日消耗时间单位（默认: d=天，默认显示）
  -h, --help                Print help";

#[derive(Debug)]
enum Action {
    Run(Command),
    Help(String),
    Version,
}

#[derive(Debug)]
enum Command {
    Ds {
        format: OutputFormat,
        output: Option<String>,
    },
    Or {
        format: OutputFormat,
        output: Option<String>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum OutputFormat {
    Text,
    Json,
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match parse_args(&args) {
        Ok(Action::Help(usage)) => {
            println!("{usage}");
            ExitCode::SUCCESS
        }
        Ok(Action::Version) => {
            println!("crd {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Ok(Action::Run(cmd)) => match run(cmd) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("错误: {e:#}");
                ExitCode::FAILURE
            }
        },
        Err(msg) => {
            eprintln!("{msg}");
            ExitCode::from(2)
        }
    }
}

fn parse_args(args: &[String]) -> Result<Action, String> {
    match args.first().map(String::as_str) {
        None => Err(USAGE.to_string()),
        Some("-h" | "--help") => Ok(Action::Help(USAGE.to_string())),
        Some("-V" | "--version") => Ok(Action::Version),
        Some("help") => match args.get(1).map(String::as_str) {
            None | Some("ds") => Ok(Action::Help(USAGE.to_string())),
            Some("or") => Ok(Action::Help(OR_USAGE.to_string())),
            Some(_) => Err("错误: 无法识别的命令".to_string()),
        },
        Some("ds") => parse_balance_args(
            DS_USAGE,
            &args[1..],
            |format, output| Command::Ds { format, output },
        ),
        Some("or") => parse_balance_args(
            OR_USAGE,
            &args[1..],
            |format, output| Command::Or { format, output },
        ),
        Some(unknown) => Err(format!("错误: 无法识别的命令: {unknown}\n\n{USAGE}")),
    }
}

fn parse_balance_args(
    usage: &str,
    args: &[String],
    cmd: impl FnOnce(OutputFormat, Option<String>) -> Command,
) -> Result<Action, String> {
    let mut format = OutputFormat::Text;
    let mut output = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => return Ok(Action::Help(usage.to_string())),
            "-o" | "--output" => {
                i += 1;
                output = Some(
                    args.get(i)
                        .ok_or("错误: --output 需要一个文件路径")?
                        .clone(),
                );
            }
            "-t" | "--today" => {
                if let Some(next) = args.get(i + 1)
                    && !next.starts_with('-')
                {
                    i += 1;
                    parse_unit(next)?;
                }
            }
            "--format" => {
                i += 1;
                format = parse_format(args.get(i).ok_or("错误: --format 需要一个值 (text 或 json)")?)?;
            }
            arg if arg.starts_with("--format=") => {
                format = parse_format(&arg["--format=".len()..])?;
            }
            arg if arg.starts_with("-o=") => {
                output = Some(arg["-o=".len()..].to_string());
            }
            arg if arg.starts_with("--today=") => {
                parse_unit(&arg["--today=".len()..])?;
            }
            arg if arg.starts_with("-t=") => {
                parse_unit(&arg["-t=".len()..])?;
            }
            arg => return Err(format!("错误: 未知选项: {arg}\n\n{usage}")),
        }
        i += 1;
    }
    Ok(Action::Run(cmd(format, output)))
}

fn parse_format(s: &str) -> Result<OutputFormat, String> {
    match s {
        "text" => Ok(OutputFormat::Text),
        "json" => Ok(OutputFormat::Json),
        _ => Err(format!("错误: 无效的格式: {s}（可选值: text, json）")),
    }
}

fn parse_unit(s: &str) -> Result<(), String> {
    match s {
        "d" => Ok(()),
        _ => Err(format!("错误: 无效的时间单位: {s}（可选值: d）")),
    }
}

fn run(cmd: Command) -> Result<()> {
    let (provider, format, output): (Box<dyn Provider>, OutputFormat, Option<String>) =
        match cmd {
            Command::Ds { format, output } => (Box::new(DeepSeek), format, output),
            Command::Or { format, output } => (Box::new(OpenRouter), format, output),
        };

    let api_key = std::env::var(provider.api_key_env_name()).map_err(|_| {
        anyhow!(
            "环境变量 {} 未设置，请先导出，例如: $env:{} = \"sk-...\"",
            provider.api_key_env_name(),
            provider.api_key_env_name()
        )
    })?;

    let report = provider.fetch_balance(&api_key)?;
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
    let usage = match cache::cache_path() {
        Some(path) => {
            let usage = cache::today_usage(
                &cache::load(&path).records,
                provider.id(),
                &today,
                current,
            );
            if let Err(e) = cache::record_query(&path, provider.id(), &today, current, &currency)
            {
                eprintln!("警告: 写入缓存失败: {e:#}");
            }
            usage
        }
        None => 0.0,
    };

    let rendered = match format {
        OutputFormat::Text => render_text(&report, usage),
        OutputFormat::Json => {
            serde_json::to_string_pretty(&serde_json::json!({
                "balance": report,
                "today_usage": format!("{usage:.2}"),
            }))?
        }
    };

    match output {
        Some(path) => {
            let mut file = File::create(&path)
                .with_context(|| format!("无法创建文件 {path}"))?;
            writeln!(file, "{rendered}").with_context(|| format!("写入文件 {path} 失败"))?;
        }
        None => println!("{rendered}"),
    }
    Ok(())
}

fn render_text(report: &BalanceReport, usage: f64) -> String {
    let mut s = String::new();
    s.push_str(if report.is_available {
        "状态: 可用\n"
    } else {
        "状态: 不可用\n"
    });
    for info in &report.balance_infos {
        s.push_str(&format!(
            "总余额 {} {}（充值 {} / 赠送 {}）\n",
            info.total_balance, info.currency, info.topped_up_balance, info.granted_balance
        ));
    }
    if let Some(info) = report.balance_infos.first() {
        s.push_str(&format!("今日消耗 {usage:.2} {}\n", info.currency));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    fn cmd_of(action: Action) -> Command {
        match action {
            Action::Run(cmd) => cmd,
            _ => panic!("expected Run, got {:?}", action),
        }
    }

    #[test]
    fn parses_help_and_version() {
        assert!(matches!(
            parse_args(&args(&["-h"])),
            Ok(Action::Help(_))
        ));
        assert!(matches!(
            parse_args(&args(&["help", "or"])),
            Ok(Action::Help(_))
        ));
        assert!(matches!(
            parse_args(&args(&["-V"])),
            Ok(Action::Version)
        ));
    }

    #[test]
    fn parses_today_flag() {
        assert!(matches!(
            cmd_of(parse_args(&args(&["ds", "-t"])).unwrap()),
            Command::Ds { .. }
        ));
        assert!(matches!(
            cmd_of(parse_args(&args(&["or", "-t", "d"])).unwrap()),
            Command::Or { .. }
        ));
        assert!(matches!(
            cmd_of(parse_args(&args(&["ds", "--today=d"])).unwrap()),
            Command::Ds { .. }
        ));
        assert!(matches!(
            cmd_of(parse_args(&args(&["ds"])).unwrap()),
            Command::Ds { .. }
        ));
    }

    #[test]
    fn parses_format_and_output() {
        match cmd_of(parse_args(&args(&["ds", "--format=json", "-o", "x.json"])).unwrap()) {
            Command::Ds { format, output, .. } => {
                assert_eq!(format, OutputFormat::Json);
                assert_eq!(output.as_deref(), Some("x.json"));
            }
            _ => panic!("expected Ds"),
        }
    }

    #[test]
    fn rejects_bad_args() {
        assert!(parse_args(&args(&["bogus"])).is_err());
        assert!(parse_args(&args(&["ds", "-t", "w"])).is_err());
        assert!(parse_args(&args(&["ds", "--format", "xml"])).is_err());
        assert!(parse_args(&args(&["ds", "-o"])).is_err());
        assert!(parse_args(&args(&["ds", "--nope"])).is_err());
    }
}
