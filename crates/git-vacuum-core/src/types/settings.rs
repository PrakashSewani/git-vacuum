use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SettingsCategory {
    General,
    Clone,
    Sync,
    GitHub,
    Advanced,
}

impl SettingsCategory {
    pub fn label(&self) -> &'static str {
        match self {
            SettingsCategory::General => "General",
            SettingsCategory::Clone => "Clone",
            SettingsCategory::Sync => "Sync",
            SettingsCategory::GitHub => "GitHub",
            SettingsCategory::Advanced => "Advanced",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsField {
    pub key: String,
    pub label: String,
    pub value: String,
    pub kind: SettingsFieldKind,
    pub help: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SettingsFieldKind {
    Text,
    Path,
    Boolean,
    Integer { min: i64, max: i64 },
    Dropdown { options: Vec<String> },
}
