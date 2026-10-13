pub mod deepseek;
pub mod http;
pub mod openrouter;

use anyhow::{Result, anyhow};

pub(crate) fn read_api_key(env_name: &str) -> Result<String> {
    std::env::var(env_name).map_err(|_| {
        anyhow!("环境变量 {env_name} 未设置，请先导出，例如: $env:{env_name} = \"sk-...\"")
    })
}
