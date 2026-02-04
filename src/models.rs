use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DropItem {
    pub name: String,
    pub quantity: u32,
    pub value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub map: String,
    pub notes: Option<String>,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub drops: Vec<DropItem>,
}

impl Session {
    pub fn is_active(&self) -> bool {
        self.end_time.is_none()
    }

    pub fn total_value(&self) -> f64 {
        self.drops
            .iter()
            .map(|d| d.value * d.quantity as f64)
            .sum()
    }

    pub fn duration_minutes(&self) -> Option<f64> {
        let end = self.end_time?;
        let duration = end - self.start_time;
        Some(duration.num_seconds() as f64 / 60.0)
    }

    pub fn profit_per_minute(&self) -> Option<f64> {
        let minutes = self.duration_minutes()?;
        if minutes <= 0.0 {
            return None;
        }
        Some(self.total_value() / minutes)
    }
}
