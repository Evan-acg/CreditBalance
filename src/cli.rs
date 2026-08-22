const USAGE: &str = "查询 AI 平台账户余额及订阅额度

用法: crd <COMMAND> [选项]

命令:
  ds    查询 DeepSeek 余额
  or    查询 OpenRouter 余额
  gpt   查询 Codex 订阅额度及重置时间
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
    Ds {
        format: OutputFormat,
        output: Option<String>,
    },
    Or {
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

pub(crate) fn parse_args(args: &[String]) -> Result<Action, String> {
    match args.first().map(String::as_str) {
        None => Err(USAGE.to_string()),
        Some("-h" | "--help") => Ok(Action::Help(USAGE.to_string())),
        Some("-V" | "--version") => Ok(Action::Version),
        Some("help") => parse_help(args.get(1).map(String::as_str)),
        Some("ds") => parse_command_args(DS_USAGE, &args[1..], true, |format, output| {
            Command::Ds { format, output }
        }),
        Some("or") => parse_command_args(OR_USAGE, &args[1..], true, |format, output| {
            Command::Or { format, output }
        }),
        Some("gpt") => parse_command_args(GPT_USAGE, &args[1..], false, |format, output| {
            Command::Gpt { format, output }
        }),
        Some(unknown) => Err(format!("错误: 无法识别的命令: {unknown}\n\n{USAGE}")),
    }
}

fn parse_help(command: Option<&str>) -> Result<Action, String> {
    match command {
        None => Ok(Action::Help(USAGE.to_string())),
        Some("ds") => Ok(Action::Help(DS_USAGE.to_string())),
        Some("or") => Ok(Action::Help(OR_USAGE.to_string())),
        Some("gpt") => Ok(Action::Help(GPT_USAGE.to_string())),
        Some(_) => Err("错误: 无法识别的命令".to_string()),
    }
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

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|value| value.to_string()).collect()
    }

    fn cmd_of(action: Action) -> Command {
        match action {
            Action::Run(command) => command,
            _ => panic!("expected Run, got {action:?}"),
        }
    }

    #[test]
    fn parses_help_and_version() {
        assert!(matches!(parse_args(&args(&["-h"])), Ok(Action::Help(_))));
        assert!(matches!(
            parse_args(&args(&["help", "or"])),
            Ok(Action::Help(_))
        ));
        assert!(matches!(parse_args(&args(&["-V"])), Ok(Action::Version)));
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
            Command::Ds { format, output } => {
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
