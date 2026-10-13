#[derive(Debug, Clone, PartialEq)]
pub struct CodexUsageReport {
    pub plan_type: Option<String>,
    pub primary: Option<UsageWindow>,
    pub secondary: Option<UsageWindow>,
    pub credits: Option<Credits>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UsageWindow {
    pub used_percent: i32,
    pub remaining_percent: i32,
    pub window_duration_mins: Option<i64>,
    pub resets_at: Option<i64>,
}

impl UsageWindow {
    pub fn from_used_percent(
        used_percent: i32,
        window_duration_mins: Option<i64>,
        resets_at: Option<i64>,
    ) -> Self {
        Self {
            used_percent,
            remaining_percent: 100_i32.saturating_sub(used_percent).clamp(0, 100),
            window_duration_mins,
            resets_at,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Credits {
    pub has_credits: bool,
    pub unlimited: bool,
    pub balance: Option<String>,
}
