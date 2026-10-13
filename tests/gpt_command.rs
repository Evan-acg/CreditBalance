use std::process::Command;

// Given 用户已安装 crd
// When 用户查看全局帮助
// Then 帮助中列出 gpt 订阅额度命令
#[test]
fn displays_remaining_codex_quota_and_reset_times() {
    let output = Command::new(env!("CARGO_BIN_EXE_crd"))
        .arg("--help")
        .output()
        .expect("crd should start");

    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(output.status.success());
    assert!(stdout.contains("gpt   查询 Codex 订阅额度及重置时间"));
}

// Given 用户准备以 JSON 查询 Codex 额度
// When 用户查看 gpt 帮助
// Then 帮助说明支持 --format text|json
#[test]
fn renders_codex_quota_as_json() {
    let output = Command::new(env!("CARGO_BIN_EXE_crd"))
        .args(["gpt", "--help"])
        .output()
        .expect("crd should start");

    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(output.status.success());
    assert!(stdout.contains("--format <text|json>  输出格式（默认: text）"));
}

// Given 用户准备将 Codex 额度写入文件
// When 用户查看 gpt 帮助
// Then 帮助说明支持 -o/--output
#[test]
fn writes_codex_quota_to_file() {
    let output = Command::new(env!("CARGO_BIN_EXE_crd"))
        .args(["help", "gpt"])
        .output()
        .expect("crd should start");

    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(output.status.success());
    assert!(stdout.contains("-o, --output <文件>       将输出写入文件"));
}

// Given 用户查询 Codex 订阅额度
// When 用户传入仅适用于余额缓存的 --today
// Then 命令拒绝该参数并显示 gpt 用法
#[test]
fn rejects_today_for_gpt_command() {
    let output = Command::new(env!("CARGO_BIN_EXE_crd"))
        .args(["gpt", "--today"])
        .output()
        .expect("crd should start");

    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr.contains("未知选项: --today"));
    assert!(stderr.contains("用法: crd gpt"));
}

// Given PATH 和 Desktop 安装目录中都没有 Codex app-server runtime
// When 用户执行 crd gpt
// Then 命令提示可安装的 runtime，且不泄露认证凭据
#[test]
fn reports_codex_app_server_failures_safely() {
    let output = Command::new(env!("CARGO_BIN_EXE_crd"))
        .arg("gpt")
        .env("PATH", "")
        .env(
            "LOCALAPPDATA",
            std::env::temp_dir().join(format!("crd-no-desktop-{}", std::process::id())),
        )
        .output()
        .expect("crd should start");

    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(!output.status.success());
    assert!(stderr.contains("找不到 Codex app-server runtime"));
    assert!(!stderr.to_ascii_lowercase().contains("access_token"));
}
