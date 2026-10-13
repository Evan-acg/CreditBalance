use anyhow::Result;

use crate::application::Application;
use crate::application::provider_registry::ProviderRegistry;
use crate::infrastructure::codex::AppServerCodexQuotaReader;
use crate::infrastructure::persistence::json_usage_history::JsonUsageHistoryRepository;
use crate::infrastructure::providers::{deepseek::DeepSeek, openrouter::OpenRouter};

pub fn build_application() -> Result<Application> {
    let mut providers = ProviderRegistry::default();
    providers.register(Box::new(DeepSeek))?;
    providers.register(Box::new(OpenRouter))?;

    Ok(Application::new(
        providers,
        Box::new(JsonUsageHistoryRepository::default()),
        Box::new(AppServerCodexQuotaReader),
    ))
}
