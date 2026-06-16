use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub sync: SyncSettings,
    pub explorer: ExplorerSettings,
    pub appearance: AppearanceSettings,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            sync: SyncSettings::default(),
            explorer: ExplorerSettings::default(),
            appearance: AppearanceSettings::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncSettings {
    pub clone_path: String,
    pub default_concurrency: u32,
    pub default_protocol: String,
    pub auto_prune: bool,
    pub include_wikis: bool,
    pub lfs_enabled: bool,
}

impl Default for SyncSettings {
    fn default() -> Self {
        Self {
            clone_path: String::new(),
            default_concurrency: 8,
            default_protocol: "ssh".to_string(),
            auto_prune: false,
            include_wikis: false,
            lfs_enabled: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplorerSettings {
    pub skip_archived: bool,
    pub skip_forks: bool,
    pub default_source: String,
    pub sort_column: u8,
    pub sort_ascending: bool,
}

impl Default for ExplorerSettings {
    fn default() -> Self {
        Self {
            skip_archived: true,
            skip_forks: true,
            default_source: "my_repos".to_string(),
            sort_column: 2,
            sort_ascending: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppearanceSettings {
    pub color_scheme: String,
    pub compact_mode: bool,
    pub show_icons: bool,
    pub show_breadcrumbs: bool,
}

impl Default for AppearanceSettings {
    fn default() -> Self {
        Self {
            color_scheme: "default".to_string(),
            compact_mode: false,
            show_icons: true,
            show_breadcrumbs: true,
        }
    }
}
