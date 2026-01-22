use serde::{Deserialize, Serialize};

/// ModelQuotaInfo
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelQuota {
    pub name: String,
    pub percentage: i32,  // RemainingPercentage 0-100
    pub reset_time: String,
}

/// QuotaDataStruct
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaData {
    pub models: Vec<ModelQuota>,
    pub last_updated: i64,
    #[serde(default)]
    pub is_forbidden: bool,
    /// Subscription level (FREE/PRO/ULTRA)
    #[serde(default)]
    pub subscription_tier: Option<String>,
}

impl QuotaData {
    pub fn new() -> Self {
        Self {
            models: Vec::new(),
            last_updated: chrono::Utc::now().timestamp(),
            is_forbidden: false,
            subscription_tier: None,
        }
    }

    pub fn add_model(&mut self, name: String, percentage: i32, reset_time: String) {
        self.models.push(ModelQuota {
            name,
            percentage,
            reset_time,
        });
    }
}

impl Default for QuotaData {
    fn default() -> Self {
        Self::new()
    }
}
