//! 命令行参数解析。

const GPT_USAGE: &str = "用法: crd gpt [--format <text|json>] [-o <文件>]

选项:
      --format <text|json>  输出格式（默认: text）
  -o, --output <文件>       将输出写入文件
  -h, --help                Print help";

#[derive(Debug)]
pub(crate) enum Action {
    Run(Command),
    Help(String),
    Version,
}

#[derive(Debug)]
pub(crate) enum Command {
    Balance {
        provider_id: crate::domain::balance::ProviderId,
        format: OutputFormat,
        output: Option<String>,
    },
    Gpt {
        format: OutputFormat,
        output: Option<String>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum OutputFormat {
    Text,
    Json,
}

pub(crate) fn parse_args(
    args: &[String],
    providers: &[crate::application::ports::BalanceProviderDescriptor],
) -> Result<Action, String> {
    match args.first().map(String::as_str) {
        None => Err(root_usage(providers)),
        Some("-h" | "--help") => Ok(Action::Help(root_usage(providers))),
        Some("-V" | "--version") => Ok(Action::Version),
        Some("help") => parse_help(args.get(1).map(String::as_str), providers),
        Some("gpt") => parse_command_args(GPT_USAGE, &args[1..], false, |format, output| {
            Command::Gpt { format, output }
        }),
        Some(provider_name) => {
            let Some(provider) = providers
                .iter()
                .find(|provider| provider.id.as_str() == provider_name)
            else {
                return Err(format!(
                    "错误: 无法识别的命令: {provider_name}\n\n{}",
                    root_usage(providers)
                ));
            };
            let usage = balance_usage(provider.id.as_str());
            parse_command_args(&usage, &args[1..], true, |format, output| {
                Command::Balance {
                    provider_id: provider.id.clone(),
                    format,
                    output,
                }
            })
        }
    }
}

fn parse_help(
    command: Option<&str>,
    providers: &[crate::application::ports::BalanceProviderDescriptor],
) -> Result<Action, String> {
    match command {
        None => Ok(Action::Help(root_usage(providers))),
        Some("gpt") => Ok(Action::Help(GPT_USAGE.to_string())),
        Some(provider_name) => providers
            .iter()
            .find(|provider| provider.id.as_str() == provider_name)
            .map(|provider| Action::Help(balance_usage(provider.id.as_str())))
            .ok_or_else(|| "错误: 无法识别的命令".to_string()),
    }
}

fn root_usage(providers: &[crate::application::ports::BalanceProviderDescriptor]) -> String {
    let width = providers
        .iter()
        .map(|provider| provider.id.as_str().len())
        .max()
        .unwrap_or(3)
        .max(3)
        + 2;
    let mut commands = providers
        .iter()
        .map(|provider| {
            format!(
                "  {:width$} 查询 {} 余额\n",
                provider.id.as_str(),
                provider.display_name,
                width = width
            )
        })
        .collect::<String>();
    commands.push_str(&format!(
        "  {:width$} 查询 Codex 订阅额度及重置时间\n",
        "gpt",
        width = width
    ));
    commands.push_str(&format!(
        "  {:width$} Print this message or help\n",
        "help",
        width = width
    ));
    format!(
        "查询 AI 平台账户余额及订阅额度\n\n用法: crd <COMMAND> [选项]\n\n命令:\n{commands}\n选项:\n  -h, --help     Print help\n  -V, --version  Print version"
    )
}

fn balance_usage(provider_id: &str) -> String {
    format!(
        "用法: crd {provider_id} [--format <text|json>] [-o <文件>] [-t <单位>]\n\n选项:\n      --format <text|json>  输出格式（默认: text）\n  -o, --output <文件>       将输出写入文件\n  -t, --today <单位>        今日消耗时间单位（默认: d=天，默认显示）\n  -h, --help                Print help"
    )
}

fn parse_command_args(
    usage: &str,
    args: &[String],
    allow_today: bool,
    cmd: impl FnOnce(OutputFormat, Option<String>) -> Command,
) -> Result<Action, String> {
    let mut format = OutputFormat::Text;
    let mut output = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "-h" | "--help" => return Ok(Action::Help(usage.to_string())),
            "-o" | "--output" => {
                index += 1;
                output = Some(
                    args.get(index)
                        .ok_or("错误: --output 需要一个文件路径")?
                        .clone(),
                );
            }
            "-t" | "--today" => {
                reject_today_if_unsupported(allow_today, &args[index], usage)?;
                if let Some(next) = args.get(index + 1)
                    && !next.starts_with('-')
                {
                    index += 1;
                    parse_unit(next)?;
                }
            }
            "--format" => {
                index += 1;
                format = parse_format(
                    args.get(index)
                        .ok_or("错误: --format 需要一个值 (text 或 json)")?,
                )?;
            }
            arg if arg.starts_with("--format=") => {
                format = parse_format(&arg["--format=".len()..])?;
            }
            arg if arg.starts_with("-o=") => output = Some(arg["-o=".len()..].to_string()),
            arg if arg.starts_with("--today=") => {
                reject_today_if_unsupported(allow_today, arg, usage)?;
                parse_unit(&arg["--today=".len()..])?;
            }
            arg if arg.starts_with("-t=") => {
                reject_today_if_unsupported(allow_today, arg, usage)?;
                parse_unit(&arg["-t=".len()..])?;
            }
            arg => return Err(format!("错误: 未知选项: {arg}\n\n{usage}")),
        }
        index += 1;
    }
    Ok(Action::Run(cmd(format, output)))
}

fn reject_today_if_unsupported(allow_today: bool, arg: &str, usage: &str) -> Result<(), String> {
    if allow_today {
        Ok(())
    } else {
        Err(format!("错误: 未知选项: {arg}\n\n{usage}"))
    }
}

fn parse_format(value: &str) -> Result<OutputFormat, String> {
    match value {
        "text" => Ok(OutputFormat::Text),
        "json" => Ok(OutputFormat::Json),
        _ => Err(format!("错误: 无效的格式: {value}（可选值: text, json）")),
    }
}

fn parse_unit(value: &str) -> Result<(), String> {
    match value {
        "d" => Ok(()),
        _ => Err(format!("错误: 无效的时间单位: {value}（可选值: d）")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::ports::BalanceProviderDescriptor;
    use crate::domain::balance::ProviderId;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|value| value.to_string()).collect()
    }

    fn providers() -> Vec<BalanceProviderDescriptor> {
        vec![
            BalanceProviderDescriptor {
                id: ProviderId::new("ds"),
                display_name: "DeepSeek",
            },
            BalanceProviderDescriptor {
                id: ProviderId::new("or"),
                display_name: "OpenRouter",
            },
        ]
    }

    fn parse(list: &[&str]) -> Result<Action, String> {
        parse_args(&args(list), &providers())
    }

    fn cmd_of(action: Action) -> Command {
        match action {
            Action::Run(command) => command,
            _ => panic!("expected Run, got {action:?}"),
        }
    }

    #[test]
    fn parses_help_and_version() {
        let Ok(Action::Help(usage)) = parse(&["-h"]) else {
            panic!("expected help action");
        };
        assert!(usage.contains("ds    查询 DeepSeek 余额"));
        assert!(usage.contains("or    查询 OpenRouter 余额"));
        assert!(matches!(parse(&["help", "or"]), Ok(Action::Help(_))));
        assert!(matches!(parse(&["-V"]), Ok(Action::Version)));
    }

    #[test]
    fn parses_today_flag() {
        assert!(matches!(
            cmd_of(parse(&["ds", "-t"]).unwrap()),
            Command::Balance { .. }
        ));
        assert!(matches!(
            cmd_of(parse(&["or", "-t", "d"]).unwrap()),
            Command::Balance { .. }
        ));
        assert!(matches!(
            cmd_of(parse(&["ds", "--today=d"]).unwrap()),
            Command::Balance { .. }
        ));
        assert!(matches!(
            cmd_of(parse(&["ds"]).unwrap()),
            Command::Balance { .. }
        ));
    }

    #[test]
    fn parses_format_and_output() {
        match cmd_of(parse(&["ds", "--format=json", "-o", "x.json"]).unwrap()) {
            Command::Balance {
                format,
                output,
                provider_id,
            } => {
                assert_eq!(format, OutputFormat::Json);
                assert_eq!(output.as_deref(), Some("x.json"));
                assert_eq!(provider_id.as_str(), "ds");
            }
            _ => panic!("expected balance command"),
        }
    }

    #[test]
    fn parses_any_registered_provider_without_provider_specific_routing() {
        let providers = [BalanceProviderDescriptor {
            id: ProviderId::new("custom"),
            display_name: "Custom",
        }];

        let command = cmd_of(parse_args(&args(&["custom"]), &providers).unwrap());

        assert!(matches!(
            command,
            Command::Balance { provider_id, .. } if provider_id.as_str() == "custom"
        ));
    }

    #[test]
    fn rejects_bad_args() {
        assert!(parse(&["bogus"]).is_err());
        assert!(parse(&["ds", "-t", "w"]).is_err());
        assert!(parse(&["ds", "--format", "xml"]).is_err());
        assert!(parse(&["ds", "-o"]).is_err());
        assert!(parse(&["ds", "--nope"]).is_err());
    }
}
