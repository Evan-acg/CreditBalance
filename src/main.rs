mod providers;

use std::fs::File;
use std::io::Write;
use std::process::ExitCode;

use anyhow::{anyhow, Context, Result};
use providers::deepseek::DeepSeek;
use providers::{BalanceReport, Provider};

const USAGE: &str = "查询 AI 平台账户余额

用法: crd <COMMAND> [选项]

命令:
  ds    查询 DeepSeek 余额
  help  Print this message or the help of the given subcommand(s)

选项:
  -h, --help     Print help
  -V, --version  Print version";

const DS_USAGE: &str = "用法: crd ds [--format <text|json>] [-o <文件>]

选项:
      --format <text|json>  输出格式（默认: text）
  -o, --output <文件>       将输出写入文件
  -h, --help                Print help";

enum Action {
    Run(Command),
    Help(String),
    Version,
}

enum Command {
    Ds {
        format: OutputFormat,
        output: Option<String>,
    },
}

#[derive(Clone, Copy, PartialEq)]
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
            Some(_) => Err("错误: 无法识别的命令".to_string()),
        },
        Some("ds") => parse_ds(&args[1..]),
        Some(unknown) => Err(format!("错误: 无法识别的命令: {unknown}\n\n{USAGE}")),
    }
}

fn parse_ds(args: &[String]) -> Result<Action, String> {
    let mut format = OutputFormat::Text;
    let mut output = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => return Ok(Action::Help(DS_USAGE.to_string())),
            "-o" | "--output" => {
                i += 1;
                output = Some(
                    args.get(i)
                        .ok_or("错误: --output 需要一个文件路径")?
                        .clone(),
                );
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
            arg => return Err(format!("错误: 未知选项: {arg}\n\n{DS_USAGE}")),
        }
        i += 1;
    }
    Ok(Action::Run(Command::Ds { format, output }))
}

fn parse_format(s: &str) -> Result<OutputFormat, String> {
    match s {
        "text" => Ok(OutputFormat::Text),
        "json" => Ok(OutputFormat::Json),
        _ => Err(format!("错误: 无效的格式: {s}（可选值: text, json）")),
    }
}

fn run(cmd: Command) -> Result<()> {
    let (provider, format, output): (Box<dyn Provider>, OutputFormat, Option<String>) =
        match cmd {
            Command::Ds { format, output } => (Box::new(DeepSeek), format, output),
        };

    let api_key = std::env::var(provider.api_key_env_name()).map_err(|_| {
        anyhow!(
            "环境变量 {} 未设置，请先导出，例如: $env:{} = \"sk-...\"",
            provider.api_key_env_name(),
            provider.api_key_env_name()
        )
    })?;

    let report = provider.fetch_balance(&api_key)?;
    let rendered = match format {
        OutputFormat::Text => render_text(&report),
        OutputFormat::Json => serde_json::to_string_pretty(&report)?,
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

fn render_text(report: &BalanceReport) -> String {
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
    s
}
