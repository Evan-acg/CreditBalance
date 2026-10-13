use anyhow::Result;

use crate::domain::codex_quota::CodexUsageReport;

use super::ports::CodexQuotaReader;

pub fn execute(reader: &dyn CodexQuotaReader) -> Result<CodexUsageReport> {
    reader.fetch_quota()
}
