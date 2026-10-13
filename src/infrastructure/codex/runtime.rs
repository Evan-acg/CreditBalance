use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

use anyhow::Result;

pub(crate) fn spawn_app_server() -> Result<Child> {
    let mut last_not_found = None;
    for executable in codex_candidates() {
        let mut command = Command::new(executable);
        configure_command(&mut command);
        match command.spawn() {
            Ok(child) => return Ok(child),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                last_not_found = Some(error);
            }
            Err(error) => return Err(spawn_error(error)),
        }
    }
    Err(spawn_error(last_not_found.unwrap_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, "codex")
    })))
}

fn codex_candidates() -> Vec<PathBuf> {
    #[cfg(windows)]
    {
        let mut candidates = vec![PathBuf::from("codex")];
        candidates.push(PathBuf::from("codex.cmd"));
        candidates.extend(desktop_codex_candidates());
        candidates
    }
    #[cfg(not(windows))]
    {
        vec![PathBuf::from("codex")]
    }
}

#[cfg(windows)]
fn desktop_codex_candidates() -> Vec<PathBuf> {
    let Some(local_app_data) = std::env::var_os("LOCALAPPDATA").filter(|path| !path.is_empty())
    else {
        return Vec::new();
    };
    let bin_directory = PathBuf::from(local_app_data)
        .join("OpenAI")
        .join("Codex")
        .join("bin");
    let Ok(entries) = std::fs::read_dir(bin_directory) else {
        return Vec::new();
    };

    let mut candidates: Vec<_> = entries
        .flatten()
        .map(|entry| entry.path().join("codex.exe"))
        .filter(|path| path.is_file())
        .collect();
    candidates.sort_by(|left, right| {
        let modified = |path: &PathBuf| {
            std::fs::metadata(path)
                .and_then(|metadata| metadata.modified())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
        };
        modified(right)
            .cmp(&modified(left))
            .then_with(|| left.cmp(right))
    });
    candidates
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
        anyhow::anyhow!(
            "找不到 Codex app-server runtime，请安装 Codex CLI 或 Codex Desktop 并通过 ChatGPT 登录"
        )
    } else {
        anyhow::anyhow!(error).context("无法启动 Codex app-server")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Given Codex Desktop's bundled runtime is installed and Codex CLI is absent from PATH
    // When the runtime resolver starts the app-server without the Desktop UI
    // Then the bundled executable starts successfully
    #[cfg(windows)]
    #[test]
    fn starts_desktop_bundled_runtime_without_cli_or_desktop_ui() {
        const PROBE_ENV: &str = "CRD_TEST_DESKTOP_RUNTIME_PROBE";

        if std::env::var_os(PROBE_ENV).is_some() {
            let mut child =
                spawn_app_server().expect("Desktop app-server should start without CLI");
            child.kill().expect("test app-server should be stopped");
            child.wait().expect("test app-server should exit");
            return;
        }

        let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") else {
            return;
        };
        let bundle_dir = PathBuf::from(local_app_data)
            .join("OpenAI")
            .join("Codex")
            .join("bin");
        let has_bundled_runtime = std::fs::read_dir(bundle_dir)
            .into_iter()
            .flatten()
            .filter_map(std::result::Result::ok)
            .any(|entry| entry.path().join("codex.exe").is_file());
        if !has_bundled_runtime {
            return;
        }

        let output = Command::new(
            std::env::current_exe().expect("test executable path should be available"),
        )
        .args([
            "--exact",
            "infrastructure::codex::runtime::tests::starts_desktop_bundled_runtime_without_cli_or_desktop_ui",
            "--nocapture",
        ])
        .env(PROBE_ENV, "1")
        .env("PATH", "")
        .output()
        .expect("runtime probe test should start");

        assert!(
            output.status.success(),
            "Desktop runtime probe failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
