use serde::{Deserialize, Serialize};

/// SchedulingModeEnum
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum SchedulingMode {
    /// Cachepriority (Cache-first): 尽MayLocksameAccount，Rate Limit时priorityWait，Great improvement Prompt Caching hit rate
    CacheFirst,
    /// balanceMode (Balance): LocksameAccount，Rate LimitImmediately switch to the alternativeAccount，take into accountSuccessRate andPerformance
    Balance,
    /// Performancepriority (Performance-first): 纯Round RobinMode (Round-robin)，AccountPayloadmost balanced，but don't useCache
    PerformanceFirst,
}

impl Default for SchedulingMode {
    fn default() -> Self {
        Self::Balance
    }
}

/// viscositySessionConfig
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct StickySessionConfig {
    /// CurrentSchedulingMode
    pub mode: SchedulingMode,
    /// CachepriorityModedownMaximumWaitTime (秒)
    pub max_wait_seconds: u64,
}

impl Default for StickySessionConfig {
    fn default() -> Self {
        Self {
            mode: SchedulingMode::Balance,
            max_wait_seconds: 60,
        }
    }
}
